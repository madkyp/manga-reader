// Scraper para TaurusScan — https://lectortaurus.com/ (motor Madara WordPress)

use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use super::olympus::{MangaItem, ListResult, MangaDetail, Chapter, PageItem};

static GENRE_MAP: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
fn genre_map() -> &'static Mutex<HashMap<String, String>> {
    GENRE_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

use super::urlencode;

const BASE: &str = "https://lectortaurus.com";

fn get_html(url: &str) -> Result<String, String> {
    super::get_html(url, &format!("{}/", BASE))
}

// ── Últimas actualizaciones ───────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_latest(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let url = format!("{}/manga/?order=update&page={}", BASE, page);
    let html = get_html(&url)?;
    let results = parse_manga_list(&html);
    // Taurus no emite links de paginación en el HTML — si llega página completa hay más
    let has_more = results.len() >= 12;
    Ok(ListResult { results, has_more })
}

// ── Catálogo ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_browse(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let url = format!("{}/manga/?m_orderby=alphabet&page={}", BASE, page);
    let html = get_html(&url)?;
    let results = parse_manga_list(&html);
    let has_more = results.len() >= 12;
    Ok(ListResult { results, has_more })
}

fn parse_manga_list(html: &str) -> Vec<MangaItem> {
    // Cada card: <div class="manga__item "> … manga__thumb_item … post-title …
    let href_re  = Regex::new(r#"<a\s[^>]*href="(?:https://lectortaurus\.com)?/manga/([^/"]+)/?"[^>]*>"#).unwrap();
    let img_re   = Regex::new(r#"<img[^>]+src="(https://lectortaurus[^"]+)""#).unwrap();
    let title_re = Regex::new(r#"class="post-title[^"]*"[^>]*>.*?<a[^>]*>([^<]+)</a>"#).unwrap();
    let type_re  = Regex::new(r#"<p class="manga-type">([^<]+)</p>"#).unwrap();
    let chap_re      = Regex::new(r#"(?:class="epxs"|class="chapternum")[^>]*>\s*([^<]+?)\s*<"#).unwrap();
    let chap_href_re = Regex::new(r#"<a\s+href="([^"]+/(?:chapter|capitulo|cap|ch)[^/"]+/?)"[^>]*>"#).unwrap();
    let date_re      = Regex::new(r#"<time[^>]+datetime="([^"T]+(?:T[^"]+)?)"#).unwrap();
    let date_text_re = Regex::new(r#"(?:class="chapterdate"|class="post-on font-meta")[^>]*>\s*([^<]+?)\s*<"#).unwrap();

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    // Split flexible: acepta "manga__item", "manga__item ", "manga__item flex" etc.
    let chunks: Vec<&str> = html.split(r#"manga__item"#).collect();

    for chunk in chunks.iter().skip(1) {
        let end = chunk.find(r#"manga__item"#).unwrap_or(chunk.len());
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

        let image = img_re.captures(b)
            .map(|c| c[1].to_string())
            .unwrap_or_default();

        let manga_type = type_re.captures(b)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_else(|| "Manhwa".into());

        let chaps: Vec<String> = chap_re.captures_iter(b)
            .map(|c| html_unescape(c[1].trim()))
            .take(2)
            .collect();
        let chap_hrefs: Vec<String> = chap_href_re.captures_iter(b)
            .map(|c| c[1].trim().to_string())
            .take(2)
            .collect();
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
            id: format!("taurus-{}", slug),
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

// ── Detalle de serie ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_info(id: String) -> Result<MangaDetail, String> {
    let slug = id.trim_start_matches("taurus-");
    let url = format!("{}/manga/{}/", BASE, slug);
    let html = get_html(&url)?;

    // Título
    let title_re = Regex::new(r#"<h1[^>]*class="post-title"[^>]*>\s*([^<]+)</h1>"#).unwrap();
    let title = title_re.captures(&html)
        .map(|c| html_unescape(c[1].trim()))
        .or_else(|| {
            Regex::new(r#"og:title.*?content="([^"]+)""#).unwrap()
                .captures(&html).map(|c| html_unescape(&c[1]))
        })
        .unwrap_or_else(|| slug.replace('-', " "));

    // Imagen: data-src dentro de summary_image
    let img_re = Regex::new(r#"(?s)class="summary_image".*?data-src="([^"]+)""#).unwrap();
    let img_re2 = Regex::new(r#"og:image.*?content="([^"]+)""#).unwrap();
    let image = img_re.captures(&html)
        .map(|c| c[1].trim().to_string())
        .or_else(|| img_re2.captures(&html).map(|c| c[1].to_string()))
        .unwrap_or_default();

    // Descripción: summary__content → primer <p> después de los géneros
    let desc_re = Regex::new(r#"(?s)class="summary__content"[^>]*>(.*?)</div>\s*</div>"#).unwrap();
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

    // Géneros: class="summary__content" → enlaces con rel="tag"
    let genre_link_re = Regex::new(r#"<a[^>]+rel="tag"[^>]*>([^<]+)</a>"#).unwrap();
    let genres: Vec<String> = desc_re.captures(&html)
        .map(|c| genre_link_re.captures_iter(&c[1])
            .map(|g| g[1].trim().to_string())
            .collect())
        .unwrap_or_default();

    // Estado: class="manga-status" → segundo <span>
    let status_re = Regex::new(r#"class="manga-status"[^>]*>.*?<span>([^<]+)</span>"#).unwrap();
    let status = status_re.captures(&html)
        .map(|c| translate_status(c[1].trim()))
        .unwrap_or_else(|| "Activo".into());

    // Capítulos: dividimos el HTML por bloques "wp-manga-chapter" y extraemos
    // href, título y fecha de cada bloque individualmente.
    // El título puede estar en una línea separada del <a>, por eso no usamos [^<\n]+.
    let href_re  = Regex::new(r#"href="(https://lectortaurus\.com/[^"]+)""#).unwrap();
    let title_re = Regex::new(r#"(?s)<a\s[^>]*>([^<]+)"#).unwrap();
    let date_re  = Regex::new(r#"chapter-release-date[^>]*>.*?<i>([^<]+)</i>"#).unwrap();

    let mut chapters: Vec<Chapter> = Vec::new();
    let mut seen_ids = HashSet::new();

    for block in html.split("wp-manga-chapter").skip(1) {
        // Acotamos el bloque al siguiente marcador para no cruzar capítulos.
        // Si no hay, limitamos a 1500 bytes pero respetando frontera UTF-8.
        let raw_end = block.find("wp-manga-chapter").unwrap_or(block.len().min(1500));
        let mut end = raw_end.min(block.len());
        while end > 0 && !block.is_char_boundary(end) { end -= 1; }
        let b = &block[..end];

        // Solo capítulos con URL de lectortaurus (ignora otros hrefs del bloque)
        let chapter_url = match href_re.captures_iter(b)
            .map(|c| c[1].trim().to_string())
            .find(|u| u.contains("/manga/"))
        {
            Some(u) => u,
            None => continue,
        };

        if !seen_ids.insert(chapter_url.clone()) { continue; }

        // Título: texto del primer <a> que tenga contenido visible
        let title = title_re.captures_iter(b)
            .map(|c| html_unescape(c[1].trim()))
            .find(|t| !t.is_empty())
            .unwrap_or_else(|| {
                // Fallback: extraer número de la URL
                chapter_url.trim_end_matches('/')
                    .rsplit('/').next().unwrap_or("")
                    .replace("capitulo-", "Capítulo ")
            });

        let date = date_re.captures(b)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        chapters.push(Chapter { id: chapter_url, title, date });
    }

    Ok(MangaDetail {
        id: id.clone(),
        title,
        description,
        status,
        image,
        authors: vec![],
        genres,
        chapters,
    })
}

// ── Páginas de un capítulo ────────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_pages(chapter_id: String, _manga_id: Option<String>) -> Result<Vec<PageItem>, String> {
    let html = get_html(&chapter_id)?;

    // Las páginas están en <div class="page-break ..."><img data-src="URL">
    // El valor de data-src puede tener tabs/newlines antes de la URL — usamos (?s)
    let page_re = Regex::new(
        r#"(?s)class="page-break[^"]*"[^>]*>.*?<img[^>]+(?:data-src|src)="([^"]+)""#
    ).unwrap();

    let pages: Vec<PageItem> = page_re.captures_iter(&html)
        .filter_map(|c| {
            let url = c[1].trim().to_string();
            if url.starts_with("http") { Some(PageItem { url }) } else { None }
        })
        .collect();

    if pages.is_empty() {
        return Err("No se encontraron imágenes en el capítulo".into());
    }
    Ok(pages)
}

// ── Búsqueda por texto ────────────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_search(query: String) -> Result<ListResult, String> {
    let url = format!("{}/manga/?s={}", BASE, urlencode(&query));
    let html = get_html(&url)?;
    let results = parse_manga_list(&html);
    let has_more = html.contains("class=\"next page-numbers\"") || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

// ── Géneros ───────────────────────────────────────────────────────────────────

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
pub fn taurus_genres() -> Result<Vec<String>, String> {
    let client = super::shared_client();
    let referer = format!("{}/", BASE);

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

    if let Ok(html) = get_html(&format!("{}/manga/", BASE)) {
        let names = genres_from_html(&html);
        if !names.is_empty() {
            return Ok(names);
        }
    }

    Ok(vec![])
}

// ── Filtro por género ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn taurus_filter_genres(genres: Vec<String>, query: String, page: u32) -> Result<ListResult, String> {
    let page = page.max(1);
    let mut url = format!("{}/manga/?page={}", BASE, page);

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
    let results = parse_manga_list(&html);
    let has_more = results.len() >= 12
        || html.contains("class=\"next page-numbers\"")
        || html.contains("rel=\"next\"");
    Ok(ListResult { results, has_more })
}

// ── Utilidades ────────────────────────────────────────────────────────────────

fn translate_status(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("curso") || l.contains("ongoing") || l.contains("activo") { "Activo".into() }
    else if l.contains("complet") || l.contains("finaliz") { "Finalizado".into() }
    else if l.contains("hiatus") || l.contains("pausa") { "Pausado".into() }
    else if l.contains("cancel") || l.contains("drop") { "Cancelado".into() }
    else { s.to_string() }
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r#"<[^>]+>"#).unwrap();
    html_unescape(&re.replace_all(s, "").trim().to_string())
}

fn html_unescape(s: &str) -> String {
    let s = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
              .replace("&quot;", "\"").replace("&#039;", "'").replace("&apos;", "'")
              .replace("&nbsp;", " ").replace("&mdash;", "—").replace("&ndash;", "–")
              .replace("&lsquo;", "\u{2018}").replace("&rsquo;", "\u{2019}")
              .replace("&ldquo;", "\u{201C}").replace("&rdquo;", "\u{201D}");
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
