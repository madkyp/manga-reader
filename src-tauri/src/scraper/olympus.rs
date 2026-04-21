// Scraper para OlympusScans (olympusbiblioteca.com)
// El sitio usa Nuxt con SSR — los datos vienen embebidos en el HTML como arrays posicionales JSON

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

// URLs base del sitio y su API interna
const BASE: &str = "https://olympusbiblioteca.com";
const DASHBOARD: &str = "https://dashboard.olympusbiblioteca.com";

// ── Cliente HTTP ──────────────────────────────────────────────────────────────

fn client() -> &'static reqwest::blocking::Client {
    super::shared_client()
}

// Cabeceras para peticiones de HTML (páginas normales del sitio)
fn headers_html() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".parse().unwrap());
    h.insert("Accept-Language", "es-ES,es;q=0.9".parse().unwrap());
    h
}

// Cabeceras para peticiones a la API JSON del dashboard
// Incluye Origin y Referer para que el servidor acepte las peticiones cross-origin
fn headers_api() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert("Accept", "application/json".parse().unwrap());
    h.insert("Accept-Language", "es-ES,es;q=0.9".parse().unwrap());
    h.insert("Origin", BASE.parse().unwrap());
    h.insert("Referer", format!("{}/", BASE).parse().unwrap());
    h
}

// Decodifica entidades HTML en URLs (&amp; → &)
fn clean_url(url: &str) -> String {
    url.replace("&amp;", "&")
}

// ── Tipos de datos (structs serializables) ────────────────────────────────────

// Item de manga para listas (browse / latest)
#[derive(Serialize, Deserialize, Clone)]
pub struct MangaItem {
    pub id: String,
    pub title: String,
    pub image: String,
    pub status: String,
    #[serde(rename = "type")]
    pub manga_type: String,
    /// Capítulo más reciente (vacío si no se conoce)
    #[serde(default)]
    pub chapter: String,
    /// Fecha ISO del capítulo más reciente
    #[serde(default)]
    pub date: String,
    /// Capítulo anterior (para el segundo slot)
    #[serde(default)]
    pub chapter2: String,
    /// Fecha ISO del capítulo anterior
    #[serde(default)]
    pub date2: String,
    /// URL/ID directo al capítulo más reciente
    #[serde(default)]
    pub chapter_id: String,
    /// URL/ID directo al capítulo anterior
    #[serde(default)]
    pub chapter2_id: String,
}

// Respuesta paginada de lista de mangas
#[derive(Serialize, Deserialize)]
pub struct ListResult {
    pub results: Vec<MangaItem>,
    #[serde(rename = "hasMore")]
    pub has_more: bool, // true si hay más páginas disponibles
}

// Capítulo individual de una serie
#[derive(Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,    // ID numérico del capítulo (usado en la URL)
    pub title: String, // "Capítulo 42"
    pub date: String,  // Fecha de publicación
}

// Información completa de una serie (detalle + capítulos)
#[derive(Serialize, Deserialize)]
pub struct MangaDetail {
    pub id: String,
    pub title: String,
    pub description: String,   // Sinopsis
    pub status: String,        // "Activo", "Finalizado", etc.
    pub image: String,         // URL de la portada
    pub authors: Vec<String>,  // Siempre ["Olympus Scanlation"] por ahora
    pub genres: Vec<String>,   // Géneros extraídos del payload Nuxt
    pub chapters: Vec<Chapter>,
}

// URL de una página (imagen) dentro de un capítulo
#[derive(Serialize, Deserialize)]
pub struct PageItem {
    pub url: String,
}

// ── Comando: /latest ──────────────────────────────────────────────────────────

// Obtiene las últimas actualizaciones scrapeando /capitulos?page=N
// Retorna cards con portada, título e info básica
#[tauri::command]
pub fn get_latest(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let client = client();
    let url = format!("{}/capitulos?page={}", BASE, page);

    let html = client
        .get(&url)
        .headers(headers_html())
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;

    let results = parse_capitulos(&html);
    let last_page = detect_last_page(&html, page);

    Ok(ListResult {
        has_more: page < last_page,
        results,
    })
}

// Extrae las cards de manga de la página /capitulos usando un enfoque por chunks.
// Divide el HTML en bloques por cada cover-link (class="w-16") y dentro de cada
// bloque busca chapter links (/capitulo/ID/slug) y elementos <time>.
// Convierte un slug en título legible: elimina sufijo timestamp y pone Title Case
fn slug_to_title(slug: &str) -> String {
    // Quita sufijo -YYYYMMDD-NNNNNN (timestamp que Olympus añade a los slugs)
    let clean = {
        let bytes = slug.as_bytes();
        // Busca el patrón de 8 dígitos seguido de guión y más dígitos al final
        let mut s = slug;
        if let Some(pos) = (0..s.len().saturating_sub(8)).rev().find(|&i| {
            s[i..].starts_with('-') && s[i+1..].bytes().take(8).all(|b| b.is_ascii_digit())
        }) {
            s = &s[..pos];
        }
        let _ = bytes; // suppress warning
        s
    };
    clean.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_capitulos(html: &str) -> Vec<MangaItem> {
    // Regex del cover-link: ancla de cada card
    let cover_re  = Regex::new(r#"<a\s[^>]*href="(/series/([^"]+))"[^>]*class="[^"]*w-16[^"]*""#).unwrap();
    // src y alt se extraen de forma independiente para no depender del orden de atributos
    let img_tag_re = Regex::new(r#"<img\b([^>]+)>"#).unwrap();
    let src_attr_re = Regex::new(r#"\bsrc="([^"]+)""#).unwrap();
    let alt_attr_re = Regex::new(r#"\balt="([^"]+)""#).unwrap();
    // IDs de capítulo desde URL /capitulo/{db-id}/{slug}
    let chap_id_re   = Regex::new(r#"href="/capitulo/(\d+)/[^"]+""#).unwrap();
    // Nombre visible: "Cap. 31", "Capítulo 31.5", etc. en cualquier parte del chunk
    let chap_name_re = Regex::new(r#"(?:Cap\.?\s*|Capítulo\s+)(\d+(?:[.,]\d+)?)"#).unwrap();
    // Fecha: <time datetime="YYYY-MM-DD..."> o similares
    let date_re   = Regex::new(r#"<time[^>]+datetime="([^"T]+(?:T[^"]+)?)"[^>]*>"#).unwrap();

    // Localiza las posiciones de cada card (cada match de cover_re)
    let positions: Vec<_> = cover_re.find_iter(html).map(|m| m.start()).collect();

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for (i, &pos) in positions.iter().enumerate() {
        // El chunk de esta card termina donde empieza la siguiente (o al final del HTML)
        let end = positions.get(i + 1).copied().unwrap_or(html.len());
        let chunk = &html[pos..end];

        // Serie ID y slug desde el cover-link
        let (serie_id, _slug) = match cover_re.captures(chunk) {
            Some(c) => (c[2].split('/').last().unwrap_or("").to_string(),
                        c[2].to_string()),
            None => continue,
        };
        if serie_id.is_empty() || seen.contains(&serie_id) { continue; }
        seen.insert(serie_id.clone());

        // Imagen y título desde el primer <img> del chunk.
        // src y alt se extraen por separado para tolerar cualquier orden de atributos.
        let (img, raw_title) = if let Some(tag) = img_tag_re.captures(chunk) {
            let attrs = &tag[1];
            let src   = src_attr_re.captures(attrs).map(|c| clean_url(c[1].trim())).unwrap_or_default();
            let alt   = alt_attr_re.captures(attrs).map(|c| html_unescape(c[1].trim())).unwrap_or_default();
            (src, alt)
        } else {
            (String::new(), String::new())
        };
        // Si el título está vacío o es idéntico al slug (sin espacios), derivarlo del slug
        let title = if raw_title.is_empty() || (!raw_title.contains(' ') && raw_title.contains('-')) {
            slug_to_title(&serie_id)
        } else {
            raw_title
        };

        // IDs (para navegación) y nombres (para mostrar)
        let ch_ids: Vec<String> = chap_id_re.captures_iter(chunk)
            .map(|c| c[1].to_string()).take(2).collect();
        let ch_names: Vec<String> = chap_name_re.captures_iter(chunk)
            .map(|c| format!("Cap. {}", c[1].replace(',', "."))).take(2).collect();
        let chapter     = ch_names.first().cloned().unwrap_or_default();
        let chapter2    = ch_names.get(1).cloned().unwrap_or_default();
        let chapter_id  = ch_ids.first().cloned().unwrap_or_default();
        let chapter2_id = ch_ids.get(1).cloned().unwrap_or_default();

        // Hasta 2 fechas desde <time datetime="...">
        let dates: Vec<String> = date_re.captures_iter(chunk)
            .map(|c| c[1].trim().to_string())
            .take(2)
            .collect();
        let date  = dates.first().cloned().unwrap_or_default();
        let date2 = dates.get(1).cloned().unwrap_or_default();

        results.push(MangaItem {
            id: serie_id,
            title,
            image: img,
            status: "Activo".into(),
            manga_type: "Manhwa".into(),
            chapter,
            date,
            chapter2,
            date2,
            chapter_id,
            chapter2_id,
        });
    }
    results
}

// Detecta el número de la última página buscando el máximo de ?page=N en los links de paginación
fn detect_last_page(html: &str, current: u32) -> u32 {
    let re = Regex::new(r#"[?&]page=(\d+)"#).unwrap();
    let max = re
        .captures_iter(html)
        .filter_map(|c| c[1].parse::<u32>().ok())
        .max()
        .unwrap_or(current);
    max.max(current)
}

// ── Comando: /browse ──────────────────────────────────────────────────────────

// Obtiene el catálogo completo vía la API REST del dashboard
// La API devuelve JSON paginado con todas las series de tipo "comic"
#[tauri::command]
pub fn get_browse(page: Option<u32>) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let client = client();

    // Visitamos la página de series primero para que el servidor nos asigne cookies de sesión
    // Sin esto, la API del dashboard puede rechazar la petición
    let _ = client
        .get(format!("{}/series", BASE))
        .headers(headers_html())
        .send();

    let api_url = format!("{}/api/series?page={}&type=comic", DASHBOARD, page);
    let resp = client
        .get(&api_url)
        .headers(headers_api())
        .send()
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .map_err(|e| e.to_string())?;

    // Estructura de la respuesta: { data: { series: { data: [...], last_page: N } } }
    let series_data = &resp["data"]["series"];
    let items = series_data["data"].as_array().cloned().unwrap_or_default();
    let last_page = series_data["last_page"].as_u64().unwrap_or(1) as u32;

    let results: Vec<MangaItem> = items
        .iter()
        .map(|item| {
            let slug = item["slug"].as_str().unwrap_or("").to_string();
            // El status viene como objeto { id, name } en la API
            let status = item["status"]["name"].as_str().unwrap_or("Activo").to_string();
            MangaItem {
                id: format!("comic-{}", slug),
                title: item["name"].as_str().unwrap_or("").to_string(),
                image: item["cover"].as_str().unwrap_or("").to_string(),
                status,
                manga_type: "Manhwa".into(),
                chapter: String::new(),
                date: String::new(),
                chapter2: String::new(),
                date2: String::new(),
                chapter_id: String::new(),
                chapter2_id: String::new(),
            }
        })
        .collect();

    Ok(ListResult {
        has_more: page < last_page,
        results,
    })
}

// ── Comando: /info ────────────────────────────────────────────────────────────

// Obtiene el detalle completo de una serie usando el API del dashboard.
// El endpoint /api/series/{slug} devuelve título, sinopsis, géneros, estado y portada
// directamente en JSON — no necesitamos parsear el payload SSR de Nuxt.
#[tauri::command]
pub fn get_info(id: String) -> Result<MangaDetail, String> {
    let client = client();

    // El ID puede venir con prefijo "comic-" (desde browse) o sin él (desde latest/home).
    // El slug del dashboard es exactamente el ID sin ese prefijo.
    let slug = id.trim_start_matches("comic-").to_string();

    // ── Metadatos: título, sinopsis, géneros, estado, portada ────────────────
    let meta_url = format!("{}/api/series/{}", DASHBOARD, slug);
    let meta: Value = client
        .get(&meta_url)
        .headers(headers_api())
        .send()
        .map_err(|e| format!("Error de red: {}", e))?
        .json()
        .map_err(|e| format!("JSON inválido: {}", e))?;

    let data = &meta["data"];

    let title = data["name"].as_str().unwrap_or(&slug).trim().to_string();
    let description = data["summary"].as_str().unwrap_or("").replace("\\n", "\n");
    let image = data["cover"].as_str().unwrap_or("").to_string();
    let status = data["status"]["name"].as_str().unwrap_or("Activo").trim().to_string();
    let genres: Vec<String> = data["genres"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|g| g["name"].as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // ── Capítulos vía API paginada del dashboard ──────────────────────────────
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut ch_page = 1u32;
    loop {
        let api_url = format!(
            "{}/api/series/{}/chapters?page={}&direction=desc&type=comic",
            DASHBOARD, slug, ch_page
        );
        let resp = match client.get(&api_url).headers(headers_api()).send() {
            Ok(r) => r,
            Err(_) => break,
        };
        let ch_data: Value = match resp.json() {
            Ok(v) => v,
            Err(_) => break,
        };
        let items = match ch_data["data"].as_array() {
            Some(a) => a.clone(),
            None => break,
        };
        if items.is_empty() { break; }
        for ch in &items {
            chapters.push(Chapter {
                id: ch["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                title: format!("Capítulo {}", ch["name"].as_str().unwrap_or("")),
                date: ch["published_at"].as_str().unwrap_or("").to_string(),
            });
        }
        let last = ch_data["meta"]["last_page"].as_u64().unwrap_or(1) as u32;
        if ch_page >= last { break; }
        ch_page += 1;
    }

    // Poblar el índice de géneros con los datos ya obtenidos (sin coste extra)
    genre_index().lock().unwrap().insert(slug.clone(), genres.clone());

    Ok(MangaDetail {
        id,
        title,
        description,
        status,
        image,
        authors: vec!["Olympus Scanlation".into()],
        genres,
        chapters,
    })
}

// ── Comando: /pages ───────────────────────────────────────────────────────────

// Obtiene las URLs de las imágenes de un capítulo
// Las imágenes están embebidas en el bloque __NUXT_DATA__ del HTML
#[tauri::command]
pub fn get_pages(chapter_id: String, manga_id: Option<String>) -> Result<Vec<PageItem>, String> {
    let client = client();
    // La URL del capítulo sigue el patrón /capitulo/{id}/{manga-slug}
    let url = if let Some(ref mid) = manga_id {
        format!("{}/capitulo/{}/{}", BASE, chapter_id, mid)
    } else {
        format!("{}/capitulo/{}", BASE, chapter_id)
    };

    let html = client
        .get(&url)
        .headers(headers_html())
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;

    // Patrón que identifica URLs de imágenes del storage del dashboard
    let img_pattern =
        Regex::new(r#"dashboard\.olympusbiblioteca\.com/storage/comics/\d+/\d+/[^\s"'\\<>]+"#)
            .unwrap();

    let mut seen = HashSet::new();
    let mut result: Vec<PageItem> = Vec::new();

    // Intento 1: buscar dentro del bloque __NUXT_DATA__ (más limpio, menos ruido)
    let nuxt_re = Regex::new(r#"(?s)id="__NUXT_DATA__"[^>]*>(\[.*?\])</script>"#).unwrap();
    let search_in = if let Some(cap) = nuxt_re.captures(&html) {
        cap[1].to_string()
    } else {
        // Si no hay bloque NUXT_DATA, buscamos en todo el HTML
        html.clone()
    };

    for cap in img_pattern.captures_iter(&search_in) {
        let img = &cap[0];
        // Filtramos thumbnails/previews (terminan en (0).webp o /0)
        if img.ends_with("(0).webp") || img.ends_with("/0") {
            continue;
        }
        let url = clean_url(&format!("https://{}", img));
        if seen.insert(url.clone()) {
            result.push(PageItem { url });
        }
    }

    // Intento 2: si el bloque NUXT_DATA no tenía imágenes, buscamos en todo el HTML
    if result.is_empty() {
        for cap in img_pattern.captures_iter(&html) {
            let img = &cap[0];
            if img.ends_with("(0).webp") || img.ends_with("/0") {
                continue;
            }
            let url = clean_url(&format!("https://{}", img));
            if seen.insert(url.clone()) {
                result.push(PageItem { url });
            }
        }
    }

    Ok(result)
}

// ── Búsqueda con caché completa del catálogo ─────────────────────────────────
//
// La API de Olympus no filtra por nombre en el servidor — devuelve todo el
// catálogo paginado (56 páginas × 15 items = ~833 series). La solución:
//   1. Primera búsqueda: descarga todas las páginas en paralelo y cachea.
//   2. Búsquedas siguientes: filtro en memoria, respuesta instantánea.

// Caché global del catálogo completo. Vacío = no cargado aún.
static CATALOG_CACHE: OnceLock<Mutex<Vec<MangaItem>>> = OnceLock::new();

fn catalog_mutex() -> &'static Mutex<Vec<MangaItem>> {
    CATALOG_CACHE.get_or_init(|| Mutex::new(vec![]))
}

// Índice de géneros: slug → lista de nombres de género
// Se puebla desde get_info y desde olympus_filter_genres
static GENRE_INDEX: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();

fn genre_index() -> &'static Mutex<HashMap<String, Vec<String>>> {
    GENRE_INDEX.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Obtiene los géneros de una serie desde el índice o el API (y lo cachea).
fn fetch_genres(slug: &str) -> Vec<String> {
    {
        let idx = genre_index().lock().unwrap();
        if let Some(g) = idx.get(slug) {
            return g.clone();
        }
    }
    let url = format!("{}/api/series/{}", DASHBOARD, slug);
    let genres: Vec<String> = client()
        .get(&url)
        .headers(headers_api())
        .send()
        .ok()
        .and_then(|r| r.json::<Value>().ok())
        .and_then(|d| d["data"]["genres"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|g| g["name"].as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    genre_index().lock().unwrap().insert(slug.to_string(), genres.clone());
    genres
}

fn parse_series_array(arr: &[Value]) -> Vec<MangaItem> {
    arr.iter()
        .filter_map(|item| {
            let slug = item["slug"].as_str()?;
            if slug.is_empty() { return None; }
            let status = item["status"]["name"].as_str().unwrap_or("Activo").to_string();
            Some(MangaItem {
                id: format!("comic-{}", slug),
                title: item["name"].as_str().unwrap_or("").to_string(),
                image: item["cover"].as_str().unwrap_or("").to_string(),
                status,
                manga_type: "Manhwa".into(),
                chapter: String::new(),
                date: String::new(),
                chapter2: String::new(),
                date2: String::new(),
                chapter_id: String::new(),
                chapter2_id: String::new(),
            })
        })
        .collect()
}

// Descarga todas las páginas del catálogo en paralelo y las devuelve ordenadas
fn fetch_full_catalog() -> Vec<MangaItem> {
    let client = client();
    let _ = client.get(format!("{}/series", BASE)).headers(headers_html()).send();

    // Página 1 para saber el total de páginas
    let first_resp = match client
        .get(format!("{}/api/series", DASHBOARD))
        .query(&[("page", "1"), ("type", "comic")])
        .headers(headers_api())
        .send()
        .and_then(|r| r.json::<Value>())
    {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let series = &first_resp["data"]["series"];
    let last_page = series["last_page"].as_u64().unwrap_or(1) as u32;
    let mut all: Vec<MangaItem> = series["data"]
        .as_array()
        .map(|a| parse_series_array(a))
        .unwrap_or_default();

    if last_page > 1 {
        // Descarga las páginas restantes en paralelo
        let extra: Vec<Vec<MangaItem>> = std::thread::scope(|s| {
            let handles: Vec<_> = (2..=last_page)
                .map(|page| {
                    s.spawn(move || {
                        let page_str = page.to_string();
                        client
                            .get(format!("{}/api/series", DASHBOARD))
                            .query(&[("page", page_str.as_str()), ("type", "comic")])
                            .headers(headers_api())
                            .send()
                            .and_then(|r| r.json::<Value>())
                            .ok()
                            .and_then(|v| v["data"]["series"]["data"].as_array().cloned())
                            .map(|a| parse_series_array(&a))
                            .unwrap_or_default()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        for page_items in extra {
            all.extend(page_items);
        }
    }
    all
}

#[tauri::command]
pub async fn olympus_search(query: String) -> Result<ListResult, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(ListResult { results: vec![], has_more: false });
    }

    tokio::task::spawn_blocking(move || {
        let mutex = catalog_mutex();
        let mut cache = mutex.lock().unwrap();

        // Carga el catálogo completo si aún no está en memoria
        if cache.is_empty() {
            *cache = fetch_full_catalog();
        }

        // Filtro local por contenido (case-insensitive, cualquier posición del título)
        let results: Vec<MangaItem> = cache
            .iter()
            .filter(|item| item.title.to_lowercase().contains(&q))
            .cloned()
            .collect();

        Ok(ListResult { has_more: false, results })
    })
    .await
    .map_err(|e| format!("Error interno: {}", e))?
}

/// Devuelve todos los géneros únicos descubiertos hasta ahora.
/// Si el índice está vacío, semilla con los primeros 40 ítems del catálogo.
#[tauri::command]
pub async fn olympus_genres() -> Vec<String> {
    tokio::task::spawn_blocking(|| {
        // Asegurar que el catálogo esté cargado
        {
            let mut cache = catalog_mutex().lock().unwrap();
            if cache.is_empty() {
                *cache = fetch_full_catalog();
            }
        }

        let catalog = catalog_mutex().lock().unwrap().clone();
        let needs_seed = genre_index().lock().unwrap().len() < 5;

        if needs_seed && !catalog.is_empty() {
            let seeds: Vec<String> = catalog.iter()
                .take(40)
                .map(|i| i.id.trim_start_matches("comic-").to_string())
                .collect();
            std::thread::scope(|s| {
                let handles: Vec<_> = seeds.iter()
                    .map(|slug| { let slug = slug.as_str(); s.spawn(move || fetch_genres(slug)) })
                    .collect();
                for h in handles { let _ = h.join(); }
            });
        }

        let idx = genre_index().lock().unwrap();
        let mut seen: HashSet<String> = HashSet::new();
        for g_list in idx.values() {
            for g in g_list { seen.insert(g.clone()); }
        }
        let mut result: Vec<String> = seen.into_iter().collect();
        result.sort();
        result
    })
    .await
    .unwrap_or_default()
}

/// Filtra el catálogo por géneros (y opcionalmente por texto).
/// En la primera llamada construye el índice de géneros completo (~15-30s).
/// Las siguientes llamadas son instantáneas (todo en caché).
#[tauri::command]
pub async fn olympus_filter_genres(
    genres: Vec<String>,
    query: String,
    page: Option<u32>,
) -> Result<ListResult, String> {
    let page = page.unwrap_or(1);
    let per_page: usize = 24;

    tokio::task::spawn_blocking(move || {
        // Cargar catálogo si no está en caché
        {
            let mut cache = catalog_mutex().lock().unwrap();
            if cache.is_empty() {
                *cache = fetch_full_catalog();
            }
        }

        let catalog = catalog_mutex().lock().unwrap().clone();

        // Indexar los slugs que aún no tienen géneros en caché
        let unindexed: Vec<String> = {
            let idx = genre_index().lock().unwrap();
            catalog.iter()
                .map(|i| i.id.trim_start_matches("comic-").to_string())
                .filter(|slug| !idx.contains_key(slug))
                .collect()
        };

        if !unindexed.is_empty() {
            // Procesar en lotes de 60 hilos paralelos
            for chunk in unindexed.chunks(60) {
                let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
                std::thread::scope(|s| {
                    let handles: Vec<_> = refs.iter()
                        .map(|slug| s.spawn(move || fetch_genres(slug)))
                        .collect();
                    for h in handles { let _ = h.join(); }
                });
            }
        }

        // Snapshot del índice para el filtrado (sin lock durante el loop)
        let genre_snapshot: HashMap<String, Vec<String>> =
            genre_index().lock().unwrap().clone();

        let q = query.trim().to_lowercase();
        let genres_lower: Vec<String> = genres.iter()
            .map(|g| g.trim().to_lowercase())
            .collect();

        let filtered: Vec<MangaItem> = catalog.into_iter().filter(|item| {
            // Filtro de texto
            if !q.is_empty() && !item.title.to_lowercase().contains(&q) {
                return false;
            }
            if genres_lower.is_empty() { return true; }

            // Filtro de género
            let slug = item.id.trim_start_matches("comic-");
            match genre_snapshot.get(slug) {
                Some(item_genres) => {
                    let ig_lower: Vec<String> = item_genres.iter()
                        .map(|g| g.to_lowercase())
                        .collect();
                    genres_lower.iter().any(|gf| ig_lower.iter().any(|ig| ig.contains(gf.as_str())))
                }
                None => false,
            }
        }).collect();

        let total = filtered.len();
        let start = (page as usize - 1) * per_page;
        let end = (start + per_page).min(total);
        let results = if start < total { filtered[start..end].to_vec() } else { vec![] };

        Ok(ListResult { results, has_more: end < total })
    })
    .await
    .map_err(|e| format!("Error interno: {}", e))?
}

// ── Utilidades ────────────────────────────────────────────────────────────────

// Decodifica las entidades HTML más comunes en strings de texto
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
