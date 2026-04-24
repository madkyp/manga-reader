// Nyaa.si — buscador de torrents de anime (RSS feed)
use serde::Serialize;
use regex::Regex;

#[derive(Serialize, Clone)]
pub struct NyaaResult {
    pub id:       String,   // nyaa view ID
    pub title:    String,
    pub torrent:  String,   // URL directa al .torrent
    pub magnet:   String,   // magnet:?xt=...
    pub size:     String,
    pub seeders:  u32,
    pub leechers: u32,
    pub date:     String,
    pub category: String,
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 { return String::new(); }
    if bytes > 1_000_000_000 { return format!("{:.1} GB", bytes as f64 / 1e9); }
    if bytes > 1_000_000     { return format!("{:.0} MB", bytes as f64 / 1e6); }
    format!("{:.0} KB", bytes as f64 / 1e3)
}

/// Busca en AnimeToSho (agrega Nyaa + AniDex) — JSON API sin Cloudflare
#[tauri::command]
pub fn nyaa_search(query: String, _category: Option<String>) -> Result<Vec<NyaaResult>, String> {
    let encoded = urlencoding::encode(&query).into_owned();
    let url = format!("https://feed.animetosho.org/json?q={}&qx=1", encoded);

    let client = super::shared_client();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Red AnimeToSho: {}", e))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {} de AnimeToSho", status));
    }

    let items: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("JSON: {} — {}", e, &text[..text.len().min(200)]))?;

    let arr = items.as_array().ok_or("Respuesta inesperada de AnimeToSho")?;

    let results: Vec<NyaaResult> = arr.iter().filter_map(|item| {
        let title   = item["title"].as_str()?.to_string();
        let id      = item["id"].as_u64()?.to_string();
        let torrent = item["torrent_url"].as_str().unwrap_or("").to_string();
        let mut magnet = item["magnet_uri"].as_str().unwrap_or("").to_string();
        // Fallback: construir magnet desde info_hash si no viene magnet_uri
        if magnet.is_empty() {
            if let Some(hash) = item["info_hash"].as_str() {
                let enc = title.replace(' ', "%20");
                magnet = format!("magnet:?xt=urn:btih:{}&dn={}&tr=udp://tracker.opentrackr.org:1337/announce&tr=udp://open.stealth.si:80/announce&tr=udp://exodus.desync.com:6969/announce", hash, enc);
            }
        }
        let seeders  = item["seeders"].as_u64()
            .or_else(|| item["num_seeders"].as_u64())
            .unwrap_or(0) as u32;
        let leechers = item["leechers"].as_u64()
            .or_else(|| item["num_leechers"].as_u64())
            .unwrap_or(0) as u32;
        let size_bytes = item["total_size"].as_u64()
            .or_else(|| item["torrent_size"].as_u64())
            .or_else(|| item["size"].as_u64())
            .unwrap_or(0);
        let size = format_size(size_bytes);
        let date    = item["date_timestamp"].as_u64()
            .map(|ts| {
                let secs = ts;
                // Formato simple YYYY-MM-DD
                let days = secs / 86400;
                let epoch_days = 719162u64; // días desde año 0 hasta 1970-01-01
                let total = epoch_days + days;
                let y = total * 400 / 146097;
                format!("{}-??-??", y + 1)
            })
            .unwrap_or_default();
        let category = item["nyaa_class"].as_str().unwrap_or("").to_string();
        if torrent.is_empty() && magnet.is_empty() { return None; }
        Some(NyaaResult { id, title, torrent, magnet, size, seeders, leechers, date, category })
    }).collect();

    Ok(sort_multi_sub_first(results))
}

fn is_multi_sub(title: &str) -> bool {
    let t = title.to_lowercase();
    t.contains("[subsplease]") ||
    t.contains("[erai-raws]") ||
    t.contains("multi sub") ||
    t.contains("multisub") ||
    t.contains("multi-sub") ||
    t.contains("[multi]") ||
    t.contains("multiple sub")
}

fn sort_multi_sub_first(mut v: Vec<NyaaResult>) -> Vec<NyaaResult> {
    v.sort_by(|a, b| {
        let am = is_multi_sub(&a.title);
        let bm = is_multi_sub(&b.title);
        if am != bm { return bm.cmp(&am); }
        b.seeders.cmp(&a.seeders)
    });
    v
}

/// Busca directamente en Nyaa.si (RSS) — resultados con multi-sub primero
#[tauri::command]
pub fn nyaa_direct(query: String) -> Result<Vec<NyaaResult>, String> {
    let encoded = urlencoding::encode(&query).into_owned();
    let url = format!("https://nyaa.si/?page=rss&q={}&c=1_2&f=0", encoded);

    let client = super::shared_client();
    let text = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; RSS reader)")
        .send()
        .map_err(|e| format!("Red Nyaa: {}", e))?
        .text()
        .map_err(|e| e.to_string())?;

    let mut results = parse_rss(&text, "https://nyaa.si")?;
    results = sort_multi_sub_first(results);
    Ok(results)
}

fn parse_rss(xml: &str, base: &str) -> Result<Vec<NyaaResult>, String> {
    let domain = base.trim_end_matches('/');
    let re_item = Regex::new(r"(?s)<item>(.*?)</item>").unwrap();
    // Nyaa.si usa títulos planos; otros mirrors pueden usar CDATA
    let re_title    = Regex::new(r"<title>(?:<!\[CDATA\[(.*?)\]\]>|([^<]*))</title>").unwrap();
    let re_link     = Regex::new(r"<guid[^>]*>(?:https?://[^/]+)?/view/(\d+)</guid>").unwrap();
    // Nyaa.si pone el .torrent en <link>; otros mirrors usan <nyaa:torrent>
    let re_torrent  = Regex::new(r"<nyaa:torrent>(.*?)</nyaa:torrent>").unwrap();
    let re_link_url = Regex::new(r"<link>(https://[^<]+\.torrent[^<]*)</link>").unwrap();
    let re_infohash = Regex::new(r"<nyaa:infoHash>(.*?)</nyaa:infoHash>").unwrap();
    let re_size     = Regex::new(r"<nyaa:size>(.*?)</nyaa:size>").unwrap();
    let re_seeders  = Regex::new(r"<nyaa:seeders>(\d+)</nyaa:seeders>").unwrap();
    let re_leechers = Regex::new(r"<nyaa:leechers>(\d+)</nyaa:leechers>").unwrap();
    let re_date     = Regex::new(r"<pubDate>(.*?)</pubDate>").unwrap();
    let re_cat      = Regex::new(r"<nyaa:category>(.*?)</nyaa:category>").unwrap();

    let mut results = Vec::new();

    for cap in re_item.captures_iter(xml) {
        let item = &cap[1];

        let title = re_title.captures(item)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        if title.is_empty() { continue; }

        let id = re_link.captures(item)
            .map(|c| c[1].to_string())
            .unwrap_or_default();

        // Buscar torrent URL: primero <nyaa:torrent>, luego <link> directo al .torrent
        let torrent_raw = re_torrent.captures(item)
            .map(|c| c[1].trim().to_string())
            .or_else(|| re_link_url.captures(item).map(|c| c[1].trim().to_string()))
            .unwrap_or_default();
        let torrent = if torrent_raw.starts_with("http") {
            torrent_raw
        } else if !torrent_raw.is_empty() {
            format!("{}{}", domain, torrent_raw)
        } else if !id.is_empty() {
            format!("{}/download/{}.torrent", domain, id)
        } else {
            String::new()
        };

        let infohash = re_infohash.captures(item)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        let magnet = if !infohash.is_empty() {
            let enc_title = title.replace(' ', "%20");
            format!("magnet:?xt=urn:btih:{}&dn={}", infohash, enc_title)
        } else {
            String::new()
        };

        let size = re_size.captures(item)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        let seeders = re_seeders.captures(item)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);

        let leechers = re_leechers.captures(item)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);

        let date = re_date.captures(item)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        let category = re_cat.captures(item)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        results.push(NyaaResult {
            id, title, torrent, magnet, size, seeders, leechers, date, category,
        });
    }

    Ok(results)
}

/// Descarga el .torrent y devuelve la lista de archivos dentro (nombre + tamaño)
/// Útil para saber qué archivo de video hay dentro antes de iniciar el stream
#[tauri::command]
pub fn nyaa_torrent_info(torrent_url: String) -> Result<Vec<TorrentFileInfo>, String> {
    let client = super::shared_client();
    let bytes = client.get(&torrent_url)
        .header("Referer", "https://nyaa.si/")
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;

    parse_torrent_files(&bytes)
}

#[derive(Serialize, Clone)]
pub struct TorrentFileInfo {
    pub name: String,
    pub size: u64,
    pub path: String,  // ruta completa dentro del torrent
}

// Parser mínimo de .torrent (bencoding) para extraer lista de archivos
fn parse_torrent_files(data: &[u8]) -> Result<Vec<TorrentFileInfo>, String> {
    // Busca "info" dict dentro del bencoding para sacar "files" o "name"
    let s = String::from_utf8_lossy(data);

    // Single-file torrent: busca 4:name seguido de longitud
    // Multi-file torrent: busca 5:files lista
    // Usamos un parser bencoding mínimo basado en búsqueda de patrones
    let mut files = Vec::new();

    // Intentar extraer el nombre base del torrent
    let name = extract_bencode_string(&s, "4:name").unwrap_or_default();

    // Buscar si hay lista de archivos (multi-file)
    if let Some(files_section) = find_files_section(data) {
        files = files_section;
    } else if !name.is_empty() {
        // Single file — buscar longitud
        let length = extract_bencode_int(&s, "6:length").unwrap_or(0);
        files.push(TorrentFileInfo {
            path: name.clone(),
            name: name,
            size: length,
        });
    }

    if files.is_empty() {
        return Err("No se encontraron archivos en el torrent".into());
    }

    // Filtrar solo archivos de video
    let video_exts = ["mkv", "mp4", "avi", "webm", "mov"];
    let video_files: Vec<_> = files.iter()
        .filter(|f| {
            let lower = f.name.to_lowercase();
            video_exts.iter().any(|ext| lower.ends_with(ext))
        })
        .cloned()
        .collect();

    if !video_files.is_empty() {
        Ok(video_files)
    } else {
        Ok(files)
    }
}

fn extract_bencode_string(s: &str, key: &str) -> Option<String> {
    let pos = s.find(key)?;
    let rest = &s[pos + key.len()..];
    // Formato: N:contenido
    let colon = rest.find(':')?;
    let len: usize = rest[..colon].parse().ok()?;
    let start = colon + 1;
    if start + len <= rest.len() {
        Some(rest[start..start + len].to_string())
    } else {
        None
    }
}

fn extract_bencode_int(s: &str, key: &str) -> Option<u64> {
    let pos = s.find(key)?;
    let rest = &s[pos + key.len()..];
    // Formato: iNe
    if rest.starts_with('i') {
        let end = rest.find('e')?;
        rest[1..end].parse().ok()
    } else {
        None
    }
}

fn find_files_section(data: &[u8]) -> Option<Vec<TorrentFileInfo>> {
    // Búsqueda de "5:files" en el bencoding
    let marker = b"5:files";
    let pos = data.windows(marker.len()).position(|w| w == marker)?;

    // A partir de aquí hay una lista bencoding: l...e
    // Parseamos con un parser mínimo
    let mut files = Vec::new();
    let slice = &data[pos + marker.len()..];

    if slice.first() != Some(&b'l') {
        return None;
    }

    let text = String::from_utf8_lossy(slice);
    // Buscar cada entrada "6:length i<N>e" y "4:name" o "4:path"
    // Esto es aproximado pero funciona para torrents estándar
    let re_len  = Regex::new(r"6:lengthi(\d+)e").unwrap();
    let re_path = Regex::new(r"4:path[le](\d+):([^\x00-\x08\x0b\x0e-\x1f]+)").unwrap();

    let lengths: Vec<u64> = re_len.captures_iter(&text)
        .filter_map(|c| c[1].parse().ok())
        .collect();

    let paths: Vec<String> = re_path.captures_iter(&text)
        .filter_map(|c| {
            let len: usize = c[1].parse().ok()?;
            let name = &c[2];
            if name.len() >= len { Some(name[..len].to_string()) } else { None }
        })
        .collect();

    for (i, name) in paths.iter().enumerate() {
        files.push(TorrentFileInfo {
            name: name.clone(),
            path: name.clone(),
            size: lengths.get(i).copied().unwrap_or(0),
        });
    }

    if files.is_empty() { None } else { Some(files) }
}
