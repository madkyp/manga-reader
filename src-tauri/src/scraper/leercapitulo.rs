// Scraper para LeerCapitulo — https://www.leercapitulo.co/

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use super::olympus::{MangaItem, ListResult, MangaDetail, Chapter, PageItem};

const BASE: &str = "https://www.leercapitulo.co";

fn get_html(url: &str) -> Result<String, String> {
    super::get_html(url, &format!("{}/", BASE))
}

// ── Regexes compiladas una vez ────────────────────────────────────────────────

fn re_browse_href() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"href="/manga/([^/"]+)/([^/"]+)/"[^>]*class="tooltips""#).unwrap())
}
fn re_browse_img() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Acepta tanto data-src como src para cubrir lazy-loading
    R.get_or_init(|| Regex::new(r#"(?:data-src|src)="(/covers/[^"]+)""#).unwrap())
}
fn re_browse_title() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"class="manga-newest">\s*([^<]+?)\s*</h4>"#).unwrap())
}
fn re_browse_type() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"<span>Escribe:\s*([^<]+)</span>"#).unwrap())
}
fn re_detail_title() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"class="title-manga">\s*([^<]+?)\s*</h1>"#).unwrap())
}
fn re_detail_img() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?s)class="[^"]*cover-detail[^"]*"[^>]*>.*?<img[^>]+src="([^"]+)""#).unwrap())
}
fn re_detail_desc() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?s)id="example2"[^>]*>(.*?)</p>"#).unwrap())
}
fn re_detail_meta() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?s)class="description-update"[^>]*>(.*?)</p>"#).unwrap())
}
fn re_detail_genre() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"href="/genre/[^"]+">([^<]+)</a>"#).unwrap())
}
fn re_detail_type() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"<span>Escribe:\s*([^<]+)</span>"#).unwrap())
}
fn re_detail_status() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"<span>Estado:\s*</span>\s*([^<\n]+)"#).unwrap())
}
fn re_detail_chapters() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(
        r#"<a class="xanh" href="(/leer/[^"]+)" title="[^"]*">\s*([^<]+?)\s*</a>"#
    ).unwrap())
}
fn re_array_data() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?s)id="array_data"[^>]*>([^<]+)</p>"#).unwrap())
}
fn re_strip_tags() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"<[^>]+>"#).unwrap())
}

// ── Últimas actualizaciones (homepage tendencias) ─────────────────────────────

#[tauri::command]
pub fn leercapitulo_latest(page: Option<u32>) -> Result<ListResult, String> {
    // La homepage muestra "Últimos Capitulos Agregados" con class="media mainpage-manga"
    // (la misma estructura que el catálogo, imágenes en data-src con lozad)
    let page = page.unwrap_or(1);
    if page > 1 {
        return Ok(ListResult { results: vec![], has_more: false });
    }
    let html = get_html(BASE)?;
    let results = parse_browse_list(&html);
    Ok(ListResult { results, has_more: false })
}

// ── Búsqueda por texto ────────────────────────────────────────────────────────

use super::urlencode;

#[tauri::command]
pub fn leercapitulo_search(query: String) -> Result<ListResult, String> {
    let url = format!("{}/search/?keyword={}", BASE, urlencode(&query));
    let html = get_html(&url)?;
    let results = parse_browse_list(&html);
    Ok(ListResult { results, has_more: false })
}

// ── Catálogo alfabético ───────────────────────────────────────────────────────

#[tauri::command]
pub fn leercapitulo_browse(page: Option<u32>) -> Result<ListResult, String> {
    let letters: &[&str] = &[
        "a","b","c","d","e","f","g","h","i","j","k","l","m",
        "n","o","p","q","r","s","t","u","v","w","x","y","z","0-9"
    ];
    let page = page.unwrap_or(1) as usize;
    if page < 1 || page > letters.len() {
        return Ok(ListResult { results: vec![], has_more: false });
    }

    let letter = letters[page - 1];
    let url = format!("{}/initial/{}/", BASE, letter);

    // Error por letra no es fatal: devuelve vacío y continúa con la siguiente
    let html = match get_html(&url) {
        Ok(h) => h,
        Err(_) => return Ok(ListResult { results: vec![], has_more: page < letters.len() }),
    };

    let results = parse_browse_list(&html);
    let has_more = page < letters.len();
    Ok(ListResult { results, has_more })
}

fn parse_browse_list(html: &str) -> Vec<MangaItem> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    let chunks: Vec<&str> = html.split(r#"class="media mainpage-manga""#).collect();

    for chunk in chunks.iter().skip(1) {
        let raw_end = chunk.find(r#"class="media mainpage-manga""#).unwrap_or(chunk.len().min(3000));
        // Asegura frontera UTF-8 válida (evita panic si el cap cae dentro de un char multibyte)
        let mut end = raw_end.min(chunk.len());
        while end > 0 && !chunk.is_char_boundary(end) { end -= 1; }
        let b = &chunk[..end];

        let (manga_id, slug) = match re_browse_href().captures(b) {
            Some(c) => (c[1].to_string(), c[2].to_string()),
            None => continue,
        };
        if manga_id.is_empty() || seen.contains(&manga_id) { continue; }
        seen.insert(manga_id.clone());

        let title = re_browse_title().captures(b)
            .map(|c| html_unescape(c[1].trim()))
            .unwrap_or_else(|| slug.replace('-', " "));

        let image = re_browse_img().captures(b)
            .map(|c| format!("{}{}", BASE, &c[1]))
            .unwrap_or_default();

        let manga_type = re_browse_type().captures(b)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_else(|| "Manga".into());

        results.push(MangaItem {
            id: format!("leer-{}/{}", manga_id, slug),
            title,
            image,
            status: "Activo".into(),
            manga_type,
            chapter: String::new(),
            date: String::new(),
            chapter2: String::new(),
            date2: String::new(),
            chapter_id: String::new(),
            chapter2_id: String::new(),
        });
    }
    results
}

// ── Detalle de serie ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn leercapitulo_info(id: String) -> Result<MangaDetail, String> {
    let path = id.trim_start_matches("leer-");
    let url = format!("{}/manga/{}/", BASE, path);
    let html = get_html(&url)?;

    let title = re_detail_title().captures(&html)
        .map(|c| html_unescape(c[1].trim()))
        .unwrap_or_else(|| path.split('/').last().unwrap_or(path).replace('-', " "));

    let image = re_detail_img().captures(&html)
        .map(|c| {
            let src = c[1].to_string();
            if src.starts_with('/') { format!("{}{}", BASE, src) } else { src }
        })
        .unwrap_or_default();

    let description = re_detail_desc().captures(&html)
        .map(|c| strip_tags(c[1].trim()))
        .unwrap_or_default();

    let meta_html = re_detail_meta().captures(&html)
        .map(|c| c[1].to_string())
        .unwrap_or_default();

    let genres: Vec<String> = re_detail_genre().captures_iter(&meta_html)
        .map(|c| c[1].trim().to_string())
        .collect();

    let manga_type_str = re_detail_type().captures(&meta_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let status = re_detail_status().captures(&meta_html)
        .map(|c| translate_status(c[1].trim()))
        .unwrap_or_else(|| "Activo".into());

    let chapters: Vec<Chapter> = re_detail_chapters().captures_iter(&html)
        .map(|c| Chapter {
            id:    format!("{}{}", BASE, c[1].trim()),
            title: html_unescape(c[2].trim()),
            date:  String::new(),
        })
        .collect();

    Ok(MangaDetail {
        id: id.clone(),
        title,
        description,
        status,
        image,
        authors: if manga_type_str.is_empty() { vec![] } else { vec![manga_type_str] },
        genres,
        chapters,
    })
}

// ── Páginas de un capítulo ────────────────────────────────────────────────────

#[tauri::command]
pub fn leercapitulo_pages(chapter_id: String) -> Result<Vec<PageItem>, String> {
    let html = get_html(&chapter_id)?;

    let encoded = re_array_data().captures(&html)
        .map(|c| c[1].trim().to_string())
        .ok_or("No se encontró array_data en el capítulo")?;

    let urls = decode_page_urls(&encoded)?;

    if urls.is_empty() {
        return Err("No se encontraron imágenes en el capítulo".into());
    }

    Ok(urls.into_iter().map(|url| PageItem { url }).collect())
}

// ── Decodificación de imágenes (sustitución + base64) ─────────────────────────

fn decode_page_urls(encoded: &str) -> Result<Vec<String>, String> {
    let s1 = "EzCIUe3plcrfxuv9hKOsVtkTA6ZjaXRQJ0wWqb5D8gm1nG7LoH2dFyNYB4PiMS";
    let s2 = "xXHbvV7snRpMFkrUPqlS4BzG3jg1aYC5WJ0wcZiLtoAyedQ8D2fTNOI9Eu6mhK";

    let sub_map: HashMap<char, char> = s2.chars().zip(s1.chars()).collect();

    let substituted: String = encoded.chars().map(|c| {
        if c.is_ascii_alphanumeric() {
            *sub_map.get(&c).unwrap_or(&c)
        } else {
            c
        }
    }).collect();

    let decoded = STANDARD.decode(&substituted)
        .map_err(|e| format!("Error decodificando base64: {}", e))?;

    let text = String::from_utf8(decoded)
        .map_err(|e| format!("Error decodificando UTF-8: {}", e))?;

    Ok(text.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| s.starts_with("http"))
        .collect())
}

// ── Utilidades ────────────────────────────────────────────────────────────────

fn translate_status(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("ongoing") || l.contains("activo") || l.contains("curso") { "Activo".into() }
    else if l.contains("complete") || l.contains("finaliz") { "Finalizado".into() }
    else if l.contains("hiatus") || l.contains("pausa") { "Pausado".into() }
    else if l.contains("cancel") || l.contains("drop") { "Cancelado".into() }
    else { s.to_string() }
}

fn strip_tags(s: &str) -> String {
    html_unescape(&re_strip_tags().replace_all(s, "").trim().to_string())
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
