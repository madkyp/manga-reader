// Scraper para AnimeFLV (www4.animeflv.net)
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

const BASE: &str = "https://www4.animeflv.net";

fn get(url: &str) -> Result<String, String> {
    let resp = super::shared_client()
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
        .header("Referer", BASE)
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}

fn abs_img(src: &str) -> String {
    if src.starts_with("http") {
        src.to_string()
    } else if src.starts_with('/') {
        format!("{}{}", BASE, src)
    } else {
        src.to_string()
    }
}

use super::urlencode;

fn slug_from_ep_href(href: &str) -> String {
    let path = href.trim_start_matches("/ver/");
    if let Some(pos) = path.rfind('-') {
        let after = &path[pos + 1..];
        if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
            return path[..pos].to_string();
        }
    }
    path.to_string()
}

// ── Tipos ──────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct AnimeItem {
    pub id: String,
    pub title: String,
    pub image: String,
    #[serde(rename = "type")]
    pub anime_type: String,
    pub episode: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AnimeEpisode {
    pub number: u32,
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AnimeDetail {
    pub id: String,
    pub title: String,
    pub image: String,
    pub synopsis: String,
    #[serde(rename = "type")]
    pub anime_type: String,
    pub status: String,
    pub genres: Vec<String>,
    pub episodes: Vec<AnimeEpisode>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AnimeStream {
    pub server: String,
    pub url: String,
}

// ── AniList ───────────────────────────────────────────────────────────────────

const ANILIST: &str = "https://graphql.anilist.co";

// Dos queries separadas:
// 1. PEAK: obtiene el episodio más alto emitido con sort TIME_DESC perPage:1
// 2. DATES: fechas recientes (últimas 2 páginas DESC) para mostrar en UI
const PEAK_QUERY: &str = r#"
query($s:String){
  Media(search:$s,type:ANIME){
    episodes
    nextAiringEpisode{ episode }
    airingSchedule(notYetAired:false,sort:TIME_DESC,perPage:1){
      nodes{ episode airingAt }
    }
  }
}"#;

const DATES_QUERY: &str = r#"
query($s:String,$page:Int){
  Media(search:$s,type:ANIME){
    airingSchedule(notYetAired:false,sort:TIME_DESC,page:$page,perPage:50){
      pageInfo{ hasNextPage }
      nodes{ episode airingAt }
    }
  }
}"#;

fn ts_to_date(ts: i64) -> String {
    // Convierte Unix timestamp a "DD Mon AAAA"
    let months = ["Ene","Feb","Mar","Abr","May","Jun","Jul","Ago","Sep","Oct","Nov","Dic"];
    let mut t = ts;
    let secs_per_day = 86400i64;
    // Días desde epoch
    let mut days = t / secs_per_day;
    t %= secs_per_day;
    // Zeller simplificado desde 1970-01-01
    let mut year = 1970i64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [31i64,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31];
    let mut month = 0usize;
    for &md in &month_days {
        if days < md { break; }
        days -= md;
        month += 1;
    }
    format!("{} {} {}", days + 1, months[month], year)
}

#[tauri::command]
pub fn anilist_episode_dates(title: String) -> Result<String, String> {
    let client = super::shared_client();
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut max_aired: u64 = 0;

    // ── Paso 1: obtener el episodio más alto emitido con una sola petición ──
    // TIME_DESC perPage:1 → el primer nodo ES el episodio más reciente.
    {
        let body = serde_json::json!({
            "query": PEAK_QUERY,
            "variables": { "s": &title }
        });
        if let Ok(resp) = client
            .post(ANILIST)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
        {
            if let Ok(data) = resp.json::<serde_json::Value>() {
                let media = &data["data"]["Media"];

                // Series completadas: campo episodes directo
                if let Some(n) = media["episodes"].as_u64() {
                    max_aired = max_aired.max(n);
                }
                // Series en curso: próximo episodio − 1
                if let Some(next) = media["nextAiringEpisode"]["episode"].as_u64() {
                    if next > 1 { max_aired = max_aired.max(next - 1); }
                }
                // Último emitido según el schedule (más fiable para series largas)
                if let Some(nodes) = media["airingSchedule"]["nodes"].as_array() {
                    for node in nodes {
                        if let Some(ep) = node["episode"].as_u64() {
                            max_aired = max_aired.max(ep);
                        }
                    }
                }
            }
        }
    }

    // ── Paso 2: obtener fechas recientes (hasta 4 páginas DESC = últimos 200 eps) ──
    for page in 1u32..=4 {
        let body = serde_json::json!({
            "query": DATES_QUERY,
            "variables": { "s": &title, "page": page }
        });
        let Ok(resp) = client
            .post(ANILIST)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
        else { break };
        if !resp.status().is_success() { break; }
        let Ok(data) = resp.json::<serde_json::Value>() else { break };
        let schedule = &data["data"]["Media"]["airingSchedule"];

        if let Some(nodes) = schedule["nodes"].as_array() {
            for node in nodes {
                if let (Some(ep), Some(ts)) = (node["episode"].as_u64(), node["airingAt"].as_i64()) {
                    map.insert(ep.to_string(), ts_to_date(ts));
                    max_aired = max_aired.max(ep);
                }
            }
        }
        let has_next = schedule["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false);
        if !has_next { break; }
    }

    #[derive(serde::Serialize)]
    struct AniListResult {
        dates: std::collections::HashMap<String, String>,
        max_aired: u64,
    }
    serde_json::to_string(&AniListResult { dates: map, max_aired }).map_err(|e| e.to_string())
}

// ── Comandos ───────────────────────────────────────────────────────────────────

pub fn fetch_latest_items() -> Result<Vec<AnimeItem>, String> {
    let html = get(&format!("{}/", BASE))?;
    let doc  = Html::parse_document(&html);

    let li_sel  = Selector::parse("ul.ListEpisodios li a").unwrap();
    let img_sel = Selector::parse("span.Image img").unwrap();
    let cap_sel = Selector::parse("span.Capi").unwrap();
    let ttl_sel = Selector::parse("strong.Title").unwrap();

    let mut items: Vec<AnimeItem> = Vec::new();

    for a in doc.select(&li_sel) {
        let href = a.value().attr("href").unwrap_or("");

        let image = a.select(&img_sel).next()
            .and_then(|e| e.value().attr("src"))
            .map(|s| abs_img(&s.replace("/thumbs/", "/covers/")))
            .unwrap_or_default();

        let title = a.select(&ttl_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let episode = a.select(&cap_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let id = slug_from_ep_href(href);
        if id.is_empty() || title.is_empty() { continue; }

        items.push(AnimeItem { id, title, image, anime_type: "TV".to_string(), episode });
    }

    Ok(items)
}

#[tauri::command]
pub fn animeflv_latest() -> Result<String, String> {
    let items = fetch_latest_items()?;
    serde_json::to_string(&items).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn animeflv_browse(page: Option<u32>, order: Option<String>) -> Result<String, String> {
    let p = page.unwrap_or(1);
    let ord = order.as_deref().unwrap_or("updated");
    let url = format!("{}/browse?order={}&page={}", BASE, ord, p);
    let items = parse_list(&get(&url)?)?;
    serde_json::to_string(&items).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn animeflv_search(query: String) -> Result<String, String> {
    let url = format!("{}/browse?q={}", BASE, urlencode(&query));
    let items = parse_list(&get(&url)?)?;
    serde_json::to_string(&items).map_err(|e| e.to_string())
}

fn parse_list(html: &str) -> Result<Vec<AnimeItem>, String> {
    let doc = Html::parse_document(html);

    let art_sel  = Selector::parse("ul.ListAnimes li article a").unwrap();
    let img_sel  = Selector::parse("div.Image figure img").unwrap();
    let ttl_sel  = Selector::parse("h3.Title").unwrap();
    let type_sel = Selector::parse("span.Type").unwrap();

    let mut items = Vec::new();

    for a in doc.select(&art_sel) {
        let href = a.value().attr("href").unwrap_or("");
        if !href.starts_with("/anime/") { continue; }
        let id = href.trim_start_matches("/anime/").to_string();

        let image = a.select(&img_sel).next()
            .and_then(|e| e.value().attr("src"))
            .map(abs_img)
            .unwrap_or_default();

        let title = a.select(&ttl_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let anime_type = a.select(&type_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "TV".to_string());

        if id.is_empty() || title.is_empty() { continue; }
        items.push(AnimeItem { id, title, image, anime_type, episode: String::new() });
    }

    Ok(items)
}

#[tauri::command]
pub fn animeflv_detail(slug: String) -> Result<String, String> {
        let html = get(&format!("{}/anime/{}", BASE, slug))?;
        let doc  = Html::parse_document(&html);

        let title = doc.select(&Selector::parse("h1.Title").unwrap()).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or(slug.clone());

        let image = doc
            .select(&Selector::parse("div.AnimeCover figure img, div.AnimeCover img").unwrap())
            .next()
            .and_then(|e| e.value().attr("src"))
            .map(abs_img)
            .unwrap_or_default();

        let synopsis = doc.select(&Selector::parse("div.Description p").unwrap()).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let anime_type = doc.select(&Selector::parse("span.Type").unwrap()).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "TV".to_string());

        let status = doc.select(&Selector::parse("p.AnmStts span").unwrap()).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let genres: Vec<String> = doc
            .select(&Selector::parse("a[href*='genre']").unwrap())
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let episodes = extract_episodes(&html, &slug);
        let detail = AnimeDetail { id: slug, title, image, synopsis, anime_type, status, genres, episodes };
        serde_json::to_string(&detail).map_err(|e| e.to_string())
}

fn extract_episodes(html: &str, slug: &str) -> Vec<AnimeEpisode> {
    let re = Regex::new(r"var episodes\s*=\s*(\[[\s\S]*?\]);").unwrap();
    if let Some(cap) = re.captures(html) {
        if let Ok(arr) = serde_json::from_str::<Vec<Vec<serde_json::Value>>>(&cap[1]) {
            let mut eps: Vec<AnimeEpisode> = arr.iter().filter_map(|ep| {
                let num = ep.first()?.as_f64()? as u32;
                Some(AnimeEpisode {
                    number: num,
                    id: format!("{}-{}", slug, num),
                })
            }).collect();
            eps.sort_by(|a, b| b.number.cmp(&a.number));
            return eps;
        }
    }
    Vec::new()
}

#[tauri::command]
pub fn animeflv_extract(url: String) -> Result<String, String> {
    let lower = url.to_lowercase();
    if lower.contains("streamtape") {
        return extract_streamtape(&url);
    }
    Err(format!("Servidor no soportado para extracción directa"))
}

fn fetch_streamtape_html(url: &str) -> Result<String, String> {
    // Construye URL embed si llega la URL de video /v/
    let embed_url = if url.contains("/v/") {
        url.replace("/v/", "/e/")
    } else {
        url.to_string()
    };

    super::shared_client()
        .get(&embed_url)
        .header("Referer", "https://www4.animeflv.net/")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
        .header("Connection", "keep-alive")
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())
}

fn extract_streamtape(url: &str) -> Result<String, String> {
    let html = fetch_streamtape_html(url)?;

    // Detecta video eliminado/expirado
    if html.contains("\"fileid\":\"nofile\"") || html.contains("\"token\":\"errorsite\"") {
        return Err("Video eliminado o expirado en Streamtape".into());
    }

    // Patrón principal: innerHTML = 'prefix' + ('obfuscated').substring(N).substring(M)
    let re = Regex::new(
        r#"robotlink['"]\)\.innerHTML\s*=\s*['"]([^'"]+)['"]\s*\+\s*\(['"]([^'"]+)['"]\)\.substring\((\d+)\)\.substring\((\d+)\)"#
    ).unwrap();

    if let Some(cap) = re.captures(&html) {
        let prefix = &cap[1];
        let inner  = &cap[2];
        let drop   = cap[3].parse::<usize>().unwrap_or(0) + cap[4].parse::<usize>().unwrap_or(0);
        if inner.len() > drop {
            let tail: String = inner.chars().skip(drop).collect();
            let mut full = format!("{}{}", prefix, tail);
            if full.starts_with("//") { full = format!("https:{}", full); }
            if !full.starts_with("http") { full = format!("https:{}", full); }
            if !full.contains("stream=1") {
                full.push_str(if full.contains('?') { "&stream=1" } else { "?stream=1" });
            }
            return Ok(full);
        }
    }

    // Fallback: busca get_video en cualquier variante del patrón
    let fallback = Regex::new(
        r#"['"]//[^'"]*streamtape\.com/get_video\?[^'"]*token=([A-Za-z0-9_\-]+)['"]\s*[;+]"#
    ).unwrap();
    if let Some(cap) = fallback.captures(&html) {
        let id_re  = Regex::new(r"[?&]id=([A-Za-z0-9_\-]+)").unwrap();
        let tok_re = Regex::new(r"token=([A-Za-z0-9_\-]+)$").unwrap();
        // Reconstruye desde el div robotlink + token del JS
        let div_re = Regex::new(r#"id="robotlink"[^>]*>([^<]+)<"#).unwrap();
        if let Some(div) = div_re.captures(&html) {
            let path = div[1].trim();
            // El path del div usa token fake; reemplaza con el del JS
            if let Some(js_tok) = cap.get(1) {
                let rebuilt = Regex::new(r"token=[A-Za-z0-9_\-]+").unwrap()
                    .replace(path, format!("token={}", js_tok.as_str()).as_str());
                let mut full = if rebuilt.starts_with("//") {
                    format!("https:{}", rebuilt)
                } else if rebuilt.starts_with('/') {
                    format!("https://streamtape.com{}", rebuilt)
                } else {
                    rebuilt.to_string()
                };
                if !full.contains("stream=1") {
                    full.push_str(if full.contains('?') { "&stream=1" } else { "?stream=1" });
                }
                return Ok(full);
            }
        }
        let _ = (id_re, tok_re); // evita warnings
    }

    Err(format!("Patrón de Streamtape no encontrado (bytes={})", html.len()))
}

#[tauri::command]
pub fn animeflv_image(url: String) -> Result<String, String> {
    use base64::{engine::general_purpose, Engine as _};
    let resp = super::shared_client()
        .get(&url)
        .header("Referer", BASE)
        .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    let b64 = general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
pub fn animeflv_streams(episode_id: String) -> Result<String, String> {
    let html = get(&format!("{}/ver/{}", BASE, episode_id))?;

    let re = Regex::new(r"var\s+videos\s*=\s*(\{[\s\S]*?\});").unwrap();
    let mut streams: Vec<AnimeStream> = Vec::new();

    if let Some(cap) = re.captures(&html) {
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&cap[1]) {
            for lang_key in ["SUB", "LAT", "ESP"] {
                if let Some(arr) = obj.get(lang_key).and_then(|v| v.as_array()) {
                    for v in arr {
                        let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let server = v.get("server").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let url = v.get("code").or_else(|| v.get("url"))
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        if url.is_empty() { continue; }
                        let name = if !title.is_empty() { title } else { server };
                        let label = if lang_key == "SUB" { name } else { format!("{} ({})", name, lang_key) };
                        if !streams.iter().any(|s: &AnimeStream| s.url == url) {
                            streams.push(AnimeStream { server: label, url });
                        }
                    }
                }
            }
        }
    }

    if streams.is_empty() {
        return Err("No se encontraron servidores para este episodio".to_string());
    }
    serde_json::to_string(&streams).map_err(|e| e.to_string())
}
