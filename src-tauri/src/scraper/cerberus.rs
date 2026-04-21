// Scraper para CerberusScans (legionscans.com/wp) — motor Madara WordPress

use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use super::olympus::{MangaItem, ListResult, MangaDetail, Chapter, PageItem};

// Mapeo nombre → slug obtenido del WP REST API
static GENRE_MAP: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
fn genre_map() -> &'static Mutex<HashMap<String, String>> {
    GENRE_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

use super::urlencode;

const BASE: &str = "https://legionscans.com/wp";

fn get_html(url: &str) -> Result<String, String> {
    super::get_html(url, &format!("{}/", BASE))
}

// ── Comando: últimas actualizaciones ─────────────────────────────────────────

#[tauri::command]
pub fn cerberus_latest(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let url = format!("{}/manga/?order=update&page={}", BASE, page);
    let html = get_html(&url)?;
    let results = parse_manga_list(&html, "cerberus-");
    let has_more = html.contains("class=\"next page-numbers\"") || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

// ── Comando: catálogo completo ────────────────────────────────────────────────

#[tauri::command]
pub fn cerberus_browse(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let url = format!("{}/manga/?order=alphabet&page={}", BASE, page);
    let html = get_html(&url)?;
    let results = parse_manga_list(&html, "cerberus-");
    let has_more = html.contains("class=\"next page-numbers\"") || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

fn parse_manga_list(html: &str, prefix: &str) -> Vec<MangaItem> {
    // Estructura: <div class="bsx"> <a href=".../manga/SLUG/" title="TITLE">
    //   <noscript><img src="URL"/></noscript>
    //   <div class="tt">TITLE</div>
    //   <span class="type Manhwa"></span>
    //   <div class="epxs">Capítulo 50</div>   ← capítulo reciente (solo en latest)
    //   <time datetime="2024-01-15">          ← fecha ISO (solo en latest)
    // Estrategia: dividir por <div class="bsx"> y parsear cada chunk
    let href_re     = Regex::new(r#"href="[^"]+/manga/([^/"]+)/?"[^>]*>"#).unwrap();
    let title_re    = Regex::new(r#"<div class="tt">\s*([^<]+?)\s*</div>"#).unwrap();
    let noscript_re = Regex::new(r#"<noscript>[^<]*<img[^>]+src="([^"]+)""#).unwrap();
    let datasrc_re  = Regex::new(r#"data-src="([^"]+)""#).unwrap();
    let type_re     = Regex::new(r#"<span class="type\s+([^"]+)""#).unwrap();
    // Patrones de capítulo: .epxs (Madara clásico), .chapternum, o texto "Cap/Chapter N"
    let chap_re      = Regex::new(r#"(?:class="epxs"|class="chapternum")[^>]*>\s*([^<]+?)\s*<"#).unwrap();
    let chap_href_re = Regex::new(r#"<a\s+href="([^"]+/(?:chapter|capitulo|cap|ch)[^/"]+/?)"[^>]*>"#).unwrap();
    // Fecha: atributo datetime ISO o texto en .chapterdate / .post-on
    let date_re      = Regex::new(r#"<time[^>]+datetime="([^"T]+(?:T[^"]+)?)"#).unwrap();
    let date_text_re = Regex::new(r#"(?:class="chapterdate"|class="post-on font-meta")[^>]*>\s*([^<]+?)\s*<"#).unwrap();

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    // Dividimos el HTML en chunks empezando por cada <div class="bsx">
    let chunks: Vec<&str> = html.split(r#"class="bsx""#).collect();

    for chunk in chunks.iter().skip(1) {
        // Sin límite artificial — tomamos todo hasta el siguiente bsx
        let end = chunk.find(r#"class="bsx""#).unwrap_or(chunk.len());
        let b = &chunk[..end];

        let slug = match href_re.captures(b) {
            Some(c) => c[1].to_string(),
            None => continue,
        };
        if slug.is_empty() || seen.contains(&slug) { continue; }
        seen.insert(slug.clone());

        let title = title_re.captures(b)
            .map(|c| html_unescape(c[1].trim()))
            .unwrap_or_else(|| slug.replace('-', " "));

        let image = noscript_re.captures(b)
            .map(|c| c[1].to_string())
            .or_else(|| datasrc_re.captures(b).map(|c| c[1].to_string()))
            .unwrap_or_default();

        let manga_type = type_re.captures(b)
            .map(|c| c[1].trim().split_whitespace().next().unwrap_or("Manhwa").to_string())
            .unwrap_or_else(|| "Manhwa".into());

        // Extrae hasta 2 capítulos recientes (slot1 = más nuevo, slot2 = anterior)
        let chaps: Vec<String> = chap_re.captures_iter(b)
            .map(|c| html_unescape(c[1].trim()))
            .take(2)
            .collect();
        // Hrefs directos a los capítulos (excluye el link de la serie en sí)
        let chap_hrefs: Vec<String> = chap_href_re.captures_iter(b)
            .map(|c| c[1].trim().to_string())
            .take(2)
            .collect();
        // Prefiere datetime ISO; si no, coge texto de fecha
        let dates: Vec<String> = if date_re.is_match(b) {
            date_re.captures_iter(b)
                .map(|c| c[1].trim().to_string())
                .take(2)
                .collect()
        } else {
            date_text_re.captures_iter(b)
                .map(|c| c[1].trim().to_string())
                .take(2)
                .collect()
        };

        results.push(MangaItem {
            id: format!("{}{}", prefix, slug),
            title,
            image,
            status: "Activo".into(),
            manga_type,
            chapter:     chaps.first().cloned().unwrap_or_default(),
            date:        dates.first().cloned().unwrap_or_default(),
            chapter2:    chaps.get(1).cloned().unwrap_or_default(),
            date2:       dates.get(1).cloned().unwrap_or_default(),
            chapter_id:  chap_hrefs.first().cloned().unwrap_or_default(),
            chapter2_id: chap_hrefs.get(1).cloned().unwrap_or_default(),
        });
    }
    results
}

// ── Comando: detalle de serie ─────────────────────────────────────────────────

#[tauri::command]
pub fn cerberus_info(id: String) -> Result<MangaDetail, String> {
    let slug = id.trim_start_matches("cerberus-");
    let url = format!("{}/manga/{}/", BASE, slug);
    let html = get_html(&url)?;

    // Título: <h1 class="entry-title" ...>TITLE</h1>
    let title_re = Regex::new(r#"<h1[^>]*class="entry-title"[^>]*>([^<]+)</h1>"#).unwrap();
    let title = title_re.captures(&html)
        .map(|c| html_unescape(c[1].trim()))
        .unwrap_or_else(|| slug.replace('-', " "));

    // Imagen: class="thumb" > img src="..."
    let img_re = Regex::new(r#"(?s)class="thumb"[^>]*>.*?<img[^>]+src="([^"]+)""#).unwrap();
    let image = img_re.captures(&html)
        .map(|c| c[1].to_string())
        .unwrap_or_default();

    // Descripción: entry-content entry-content-single > primer <p>
    let desc_re = Regex::new(r#"(?s)entry-content-single[^>]*>(.*?)</div>"#).unwrap();
    let p_re = Regex::new(r#"(?s)<p>(.*?)</p>"#).unwrap();
    let description = desc_re.captures(&html)
        .map(|c| {
            p_re.captures_iter(&c[1])
                .map(|p| strip_tags(p[1].trim()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    // Géneros: class="seriestugenre" > <a rel="tag">Género</a>
    let genres_re = Regex::new(r#"(?s)class="seriestugenre"[^>]*>(.*?)</div>"#).unwrap();
    let genre_link_re = Regex::new(r#"<a[^>]+>([^<]+)</a>"#).unwrap();
    let genres: Vec<String> = genres_re.captures(&html)
        .map(|c| genre_link_re.captures_iter(&c[1])
            .map(|g| g[1].trim().to_string())
            .collect())
        .unwrap_or_default();

    // Estado y autor: tabla seriestucontr — <td>Status</td><td>VALUE</td>
    let table_re = Regex::new(r#"(?s)class="seriestucontr">(.*?)</table>"#).unwrap();
    let row_re   = Regex::new(r#"(?s)<td>([^<]+)</td>\s*<td>(.*?)</td>"#).unwrap();
    let mut status = "Activo".to_string();
    let mut authors: Vec<String> = vec![];
    if let Some(table) = table_re.captures(&html) {
        for row in row_re.captures_iter(&table[1]) {
            let key = row[1].trim().to_lowercase();
            let val = strip_tags(row[2].trim());
            if key == "status" || key == "estado" {
                status = translate_status(&val);
            } else if key == "author" || key == "autor" || key == "artist" {
                if !val.is_empty() && !authors.contains(&val) {
                    authors.push(val);
                }
            }
        }
    }

    // Capítulos: <a href="URL"><span class="chapternum">Chapter 50</span><span class="chapterdate">April 13, 2026</span></a>
    let ch_re = Regex::new(
        r#"<a href="([^"]+)">\s*<span class="chapternum">([^<]+)</span>\s*<span class="chapterdate">([^<]+)</span>"#
    ).unwrap();

    let chapters: Vec<Chapter> = ch_re.captures_iter(&html)
        .map(|c| Chapter {
            id:    c[1].trim().to_string(),
            title: html_unescape(c[2].trim()),
            date:  c[3].trim().to_string(),
        })
        .collect();

    Ok(MangaDetail {
        id: id.clone(),
        title,
        description,
        status,
        image,
        authors,
        genres,
        chapters,
    })
}

// ── Comando: páginas de un capítulo ──────────────────────────────────────────

#[tauri::command]
pub fn cerberus_pages(chapter_id: String) -> Result<Vec<PageItem>, String> {
    let html = get_html(&chapter_id)?;

    let start = html.find("ts_reader.run(").ok_or("No se encontraron imágenes del capítulo")?;
    let after = &html[start + "ts_reader.run(".len()..];
    let mut depth = 0i32;
    let mut end = 0;
    for (i, c) in after.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { end = i + 1; break; } }
            _ => {}
        }
    }
    let json_str = &after[..end];
    let data: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;

    let images = data["sources"].as_array()
        .and_then(|s| s.first())
        .and_then(|s| s["images"].as_array())
        .cloned()
        .unwrap_or_default();

    Ok(images.iter()
        .filter_map(|v| v.as_str())
        .map(|url| PageItem { url: url.to_string() })
        .collect())
}

// ── Búsqueda por texto ────────────────────────────────────────────────────────

#[tauri::command]
pub fn cerberus_search(query: String) -> Result<ListResult, String> {
    let url = format!("{}/manga/?s={}", BASE, urlencode(&query));
    let html = get_html(&url)?;
    let results = parse_manga_list(&html, "cerberus-");
    let has_more = html.contains("class=\"next page-numbers\"") || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

// ── Géneros ───────────────────────────────────────────────────────────────────

// Rellena el mapa nombre→slug y devuelve nombres ordenados a partir de un array JSON
fn genres_from_json(arr: &[Value]) -> Vec<String> {
    let mut map = genre_map().lock().unwrap();
    map.clear();
    let mut names = Vec::new();
    for g in arr {
        let name = html_unescape(g["name"].as_str().unwrap_or("").trim());
        let slug = g["slug"].as_str().unwrap_or("").trim().to_string();
        if !name.is_empty() && !slug.is_empty() {
            map.insert(name.clone(), slug);
            names.push(name);
        }
    }
    names.sort();
    names
}

// Extrae géneros parseando links /manga-genre/SLUG/ del HTML
fn genres_from_html(html: &str) -> Vec<String> {
    let re = Regex::new(r#"href="[^"]+/manga-genre/([^/"]+)/?"[^>]*>([^<\n]+)</a>"#).unwrap();
    let mut map = genre_map().lock().unwrap();
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for c in re.captures_iter(html) {
        let slug = c[1].trim().to_string();
        let name = html_unescape(c[2].trim());
        if !name.is_empty() && seen.insert(slug.clone()) {
            map.insert(name.clone(), slug);
            names.push(name);
        }
    }
    names.sort();
    names
}

#[tauri::command]
pub fn cerberus_genres() -> Result<Vec<String>, String> {
    let client = super::shared_client();
    let referer = format!("{}/", BASE);

    // Estrategia 1: WP REST API — Madara registra la taxonomía como "wp-manga-genre"
    for tax in &["wp-manga-genre", "manga-genre"] {
        let url = format!("{}/wp-json/wp/v2/{}?per_page=100&_fields=name,slug", BASE, tax);
        if let Ok(resp) = client.get(&url).header("Referer", &referer).send() {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<Value>() {
                    if let Some(arr) = data.as_array() {
                        if !arr.is_empty() {
                            return Ok(genres_from_json(arr));
                        }
                    }
                }
            }
        }
    }

    // Estrategia 2: scrape de la página del catálogo (ya estamos haciendo esta petición para browse)
    if let Ok(html) = get_html(&format!("{}/manga/", BASE)) {
        let names = genres_from_html(&html);
        if !names.is_empty() {
            return Ok(names);
        }
    }

    // Si ninguna estrategia funcionó devolvemos lista vacía (sin error)
    Ok(vec![])
}

// ── Filtro por género ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn cerberus_filter_genres(genres: Vec<String>, query: String, page: u32) -> Result<ListResult, String> {
    let page = page.max(1);
    let mut url = format!("{}/manga/?page={}", BASE, page);

    // Convierte nombres a slugs (preferimos el mapa del REST API, si no derivamos)
    let map = genre_map().lock().unwrap();
    for name in &genres {
        let slug = map.get(name)
            .cloned()
            .unwrap_or_else(|| name.to_lowercase().replace(' ', "-"));
        url.push_str(&format!("&genre[]={}", urlencode(&slug)));
    }
    drop(map);

    if !query.is_empty() {
        url.push_str(&format!("&s={}", urlencode(&query)));
    }
    let html = get_html(&url)?;
    let results = parse_manga_list(&html, "cerberus-");
    let has_more = html.contains("class=\"next page-numbers\"") || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

// ── Utilidades ────────────────────────────────────────────────────────────────

fn translate_status(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("ongoing") || l.contains("activo")      { "Activo".into() }
    else if l.contains("completed") || l.contains("fin")  { "Finalizado".into() }
    else if l.contains("hiatus") || l.contains("pausa")   { "Pausado".into() }
    else if l.contains("dropped") || l.contains("cancel") { "Cancelado".into() }
    else { s.to_string() }
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r#"<[^>]+>"#).unwrap();
    html_unescape(&re.replace_all(s, "").trim().to_string())
}

fn html_unescape(s: &str) -> String {
    // Entidades nombradas comunes
    let s = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
              .replace("&quot;", "\"").replace("&#039;", "'").replace("&apos;", "'")
              .replace("&nbsp;", " ").replace("&mdash;", "—").replace("&ndash;", "–")
              .replace("&lsquo;", "\u{2018}").replace("&rsquo;", "\u{2019}")
              .replace("&ldquo;", "\u{201C}").replace("&rdquo;", "\u{201D}");
    // Entidades numéricas decimales &#NNNN; y hexadecimales &#xNNNN;
    let re = Regex::new(r"&#(?:x([0-9a-fA-F]+)|([0-9]+));").unwrap();
    re.replace_all(&s, |caps: &regex::Captures| {
        let code = if let Some(h) = caps.get(1) {
            u32::from_str_radix(h.as_str(), 16).ok()
        } else {
            caps.get(2).and_then(|d| d.as_str().parse::<u32>().ok())
        };
        code.and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| caps[0].to_string())
    }).into_owned()
}
