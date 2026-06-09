// MangaDex — API oficial JSON (api.mangadex.org). Manga japonés con
// traducciones al español. Sin Cloudflare; filtramos originalLanguage=ja para
// dejar el manhwa/manhua a las otras fuentes.
use super::olympus::{Chapter, ListResult, MangaDetail, MangaItem, PageItem};

const API: &str = "https://api.mangadex.org";
const UPLOADS: &str = "https://uploads.mangadex.org";
const LIMIT: u32 = 30;

// contentRating incluidos (excluye 'pornographic' por defecto)
const RATINGS: &str = "contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica";

fn md_get(url: &str) -> Result<serde_json::Value, String> {
    let client = super::shared_client();
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Red MangaDex: {}", e))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {} de MangaDex: {}", status, &text[..text.len().min(160)]));
    }
    serde_json::from_str(&text).map_err(|e| format!("JSON MangaDex: {}", e))
}

// Mejor título disponible: es → en → romaji → primero que haya.
fn pick_title(title: &serde_json::Value, alt: Option<&serde_json::Value>) -> String {
    for key in ["es", "es-la", "en", "ja-ro"] {
        if let Some(s) = title.get(key).and_then(|v| v.as_str()) {
            if !s.is_empty() { return s.to_string(); }
        }
    }
    if let Some(s) = title.as_object().and_then(|o| o.values().next()).and_then(|v| v.as_str()) {
        return s.to_string();
    }
    // Buscar en altTitles
    if let Some(arr) = alt.and_then(|v| v.as_array()) {
        for entry in arr {
            if let Some(s) = entry.get("es").or_else(|| entry.get("en")).and_then(|v| v.as_str()) {
                if !s.is_empty() { return s.to_string(); }
            }
        }
    }
    "Sin título".to_string()
}

fn map_status(s: &str) -> String {
    match s {
        "ongoing"   => "Activo",
        "completed" => "Finalizado",
        "hiatus"    => "Pausado",
        "cancelled" => "Cancelado",
        _ => "",
    }.to_string()
}

// Construye la URL de portada a partir de las relationships del manga.
fn cover_url(manga_id: &str, relationships: &serde_json::Value) -> String {
    if let Some(arr) = relationships.as_array() {
        for r in arr {
            if r.get("type").and_then(|v| v.as_str()) == Some("cover_art") {
                if let Some(file) = r.get("attributes").and_then(|a| a.get("fileName")).and_then(|v| v.as_str()) {
                    return format!("{}/covers/{}/{}.512.jpg", UPLOADS, manga_id, file);
                }
            }
        }
    }
    String::new()
}

fn parse_manga_item(m: &serde_json::Value) -> Option<MangaItem> {
    let id = m.get("id")?.as_str()?.to_string();
    let attrs = m.get("attributes")?;
    let title = pick_title(attrs.get("title")?, attrs.get("altTitles"));
    let image = cover_url(&id, m.get("relationships").unwrap_or(&serde_json::Value::Null));
    let status = map_status(attrs.get("status").and_then(|v| v.as_str()).unwrap_or(""));
    let chapter = attrs.get("lastChapter").and_then(|v| v.as_str())
        .filter(|s| !s.is_empty()).map(|s| format!("Cap. {}", s)).unwrap_or_default();
    let date = attrs.get("updatedAt").and_then(|v| v.as_str())
        .map(|s| s.chars().take(10).collect()).unwrap_or_default();

    Some(MangaItem {
        id: format!("mangadex-{}", id),
        title, image, status,
        manga_type: "Manga".into(),
        chapter,
        date,
        chapter2: String::new(),
        date2: String::new(),
        chapter_id: String::new(),
        chapter2_id: String::new(),
    })
}

// id "mangadex-<uuid>" → "<uuid>"
fn strip_prefix(id: &str) -> &str { id.strip_prefix("mangadex-").unwrap_or(id) }

fn list_query(page: u32, order: &str) -> String {
    let offset = (page.saturating_sub(1)) * LIMIT;
    format!(
        "{}/manga?limit={}&offset={}&availableTranslatedLanguage[]=es&availableTranslatedLanguage[]=es-la&originalLanguage[]=ja&order[{}]=desc&includes[]=cover_art&{}&hasAvailableChapters=true",
        API, LIMIT, offset, order, RATINGS
    )
}

fn parse_list(json: &serde_json::Value, page: u32) -> ListResult {
    let results: Vec<MangaItem> = json.get("data").and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(parse_manga_item).collect())
        .unwrap_or_default();
    let total = json.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let has_more = (page as u64 * LIMIT as u64) < total;
    ListResult { results, has_more }
}

#[tauri::command]
pub fn mangadex_latest(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1).max(1);
    let json = md_get(&list_query(page, "latestUploadedChapter"))?;
    Ok(parse_list(&json, page))
}

#[tauri::command]
pub fn mangadex_browse(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1).max(1);
    let json = md_get(&list_query(page, "followedCount"))?;
    Ok(parse_list(&json, page))
}

#[tauri::command]
pub fn mangadex_search(query: String) -> Result<ListResult, String> {
    let url = format!(
        "{}/manga?limit={}&title={}&availableTranslatedLanguage[]=es&availableTranslatedLanguage[]=es-la&originalLanguage[]=ja&includes[]=cover_art&{}&order[relevance]=desc",
        API, LIMIT, urlencoding::encode(&query), RATINGS
    );
    let json = md_get(&url)?;
    Ok(parse_list(&json, 1))
}

#[tauri::command]
pub fn mangadex_info(id: String) -> Result<MangaDetail, String> {
    let uuid = strip_prefix(&id).to_string();

    // ── Detalle del manga ──
    let url = format!("{}/manga/{}?includes[]=cover_art&includes[]=author&includes[]=artist", API, uuid);
    let json = md_get(&url)?;
    let data = json.get("data").ok_or("Sin datos de manga")?;
    let attrs = data.get("attributes").ok_or("Sin atributos")?;

    let title = pick_title(attrs.get("title").ok_or("Sin título")?, attrs.get("altTitles"));
    let description = attrs.get("description")
        .and_then(|d| d.get("es").or_else(|| d.get("es-la")).or_else(|| d.get("en")))
        .and_then(|v| v.as_str()).unwrap_or("").to_string();
    let status = map_status(attrs.get("status").and_then(|v| v.as_str()).unwrap_or(""));
    let image = cover_url(&uuid, data.get("relationships").unwrap_or(&serde_json::Value::Null));

    let mut authors: Vec<String> = Vec::new();
    if let Some(rels) = data.get("relationships").and_then(|v| v.as_array()) {
        for r in rels {
            let t = r.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if t == "author" || t == "artist" {
                if let Some(name) = r.get("attributes").and_then(|a| a.get("name")).and_then(|v| v.as_str()) {
                    if !authors.iter().any(|x| x == name) { authors.push(name.to_string()); }
                }
            }
        }
    }

    let genres: Vec<String> = attrs.get("tags").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|tag| {
            tag.get("attributes").and_then(|a| a.get("name"))
                .and_then(|n| n.get("es").or_else(|| n.get("en")))
                .and_then(|v| v.as_str()).map(str::to_string)
        }).collect())
        .unwrap_or_default();

    // ── Capítulos (feed paginado, es + es-la, deduplicados por nº) ──
    let chapters = fetch_chapters(&uuid)?;

    Ok(MangaDetail { id, title, description, status, image, authors, genres, chapters })
}

fn fetch_chapters(uuid: &str) -> Result<Vec<Chapter>, String> {
    use std::collections::HashSet;
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let feed_limit = 100u32;
    let mut offset = 0u32;

    loop {
        let url = format!(
            "{}/manga/{}/feed?limit={}&offset={}&translatedLanguage[]=es&translatedLanguage[]=es-la&order[chapter]=desc&order[volume]=desc&{}&includes[]=scanlation_group",
            API, uuid, feed_limit, offset, RATINGS
        );
        let json = md_get(&url)?;
        let data = match json.get("data").and_then(|d| d.as_array()) {
            Some(d) if !d.is_empty() => d.clone(),
            _ => break,
        };
        let total = json.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

        for c in &data {
            let cid = match c.get("id").and_then(|v| v.as_str()) { Some(s) => s.to_string(), None => continue };
            let a = c.get("attributes").cloned().unwrap_or(serde_json::Value::Null);
            let num = a.get("chapter").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ctitle = a.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let date = a.get("publishAt").and_then(|v| v.as_str())
                .map(|s| s.chars().take(10).collect()).unwrap_or_default();

            // Deduplicar por número de capítulo (varios grupos publican el mismo).
            // Los oneshots/sin número (num vacío) se conservan todos.
            let key = if num.is_empty() { format!("__{}", cid) } else { num.clone() };
            if !seen.insert(key) { continue; }

            let label = if !num.is_empty() {
                if ctitle.is_empty() { format!("Capítulo {}", num) }
                else { format!("Capítulo {} — {}", num, ctitle) }
            } else if !ctitle.is_empty() {
                ctitle.clone()
            } else {
                "Oneshot".to_string()
            };

            chapters.push(Chapter { id: format!("mangadex-{}", cid), title: label, date });
        }

        offset += feed_limit;
        if offset as u64 >= total || offset >= 2000 { break; }
    }

    Ok(chapters)
}

#[tauri::command]
pub fn mangadex_pages(chapter_id: String) -> Result<Vec<PageItem>, String> {
    let cid = strip_prefix(&chapter_id);
    let url = format!("{}/at-home/server/{}", API, cid);
    let json = md_get(&url)?;

    let base = json.get("baseUrl").and_then(|v| v.as_str()).ok_or("Sin baseUrl")?;
    let chapter = json.get("chapter").ok_or("Sin datos de capítulo")?;
    let hash = chapter.get("hash").and_then(|v| v.as_str()).ok_or("Sin hash")?;
    let data = chapter.get("data").and_then(|v| v.as_array()).ok_or("Sin páginas")?;

    let pages: Vec<PageItem> = data.iter().filter_map(|f| f.as_str())
        .map(|file| PageItem { url: format!("{}/data/{}/{}", base, hash, file) })
        .collect();

    if pages.is_empty() { return Err("El capítulo no tiene páginas".into()); }
    Ok(pages)
}
