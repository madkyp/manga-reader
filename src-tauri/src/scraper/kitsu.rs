// Kitsu.io API — metadatos de anime (browse, search, detalle, episodios)
// NOTA: Kitsu requiere brackets literales en query params (page[limit], fields[anime], etc.)
// reqwest::Url::parse() los acepta; .query() los codificaría y daría 403.
use serde::Serialize;

const BASE: &str = "https://kitsu.io/api/edge";

#[derive(Serialize, Clone)]
pub struct KitsuAnime {
    pub id:            String,
    pub title:         String,
    pub title_ja:      Option<String>,
    pub image:         String,
    pub cover_image:   Option<String>,
    pub synopsis:      String,
    pub anime_type:    String,
    pub status:        String,
    pub episode_count: Option<u32>,
    pub rating:        Option<String>,
    pub year:          Option<u32>,
    pub genres:        Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct KitsuEpisode {
    pub id:         String,
    pub number:     u32,
    pub title:      Option<String>,
    pub airdate:    Option<String>,
    pub length_min: Option<u32>,
    pub thumbnail:  Option<String>,
}

#[derive(Serialize)]
pub struct KitsuListResult {
    pub items:    Vec<KitsuAnime>,
    pub has_more: bool,
    pub total:    u64,
}

// ── Helper HTTP + caché ───────────────────────────────────────────────────────

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static KITSU_CACHE: OnceLock<Mutex<std::collections::HashMap<String, (Instant, serde_json::Value)>>> = OnceLock::new();
const KITSU_TTL: Duration = Duration::from_secs(300); // 5 minutos

fn kitsu_cache() -> &'static Mutex<std::collections::HashMap<String, (Instant, serde_json::Value)>> {
    KITSU_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn kitsu_get(raw_url: &str) -> Result<serde_json::Value, String> {
    let client = super::shared_client();
    let resp = client
        .get(raw_url)
        .header("Accept", "application/vnd.api+json")
        .send()
        .map_err(|e| format!("Red Kitsu: {}", e))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Err(format!("HTTP {} de Kitsu: {}", status, &text[..text.len().min(200)]));
    }

    serde_json::from_str(&text)
        .map_err(|e| format!("JSON Kitsu: {} — cuerpo: {}", e, &text[..text.len().min(200)]))
}

// Versión cacheada: sirve la respuesta anterior si tiene menos de 5 min.
fn kitsu_get_cached(raw_url: &str) -> Result<serde_json::Value, String> {
    let key = raw_url.to_string();
    {
        let cache = kitsu_cache().lock().unwrap();
        if let Some((ts, val)) = cache.get(&key) {
            if ts.elapsed() < KITSU_TTL { return Ok(val.clone()); }
        }
    }
    let val = kitsu_get(raw_url)?;
    kitsu_cache().lock().unwrap().insert(key, (Instant::now(), val.clone()));
    Ok(val)
}

// ── Parse ────────────────────────────────────────────────────────────────────

fn parse_anime(item: &serde_json::Value) -> Option<KitsuAnime> {
    let id    = item.get("id")?.as_str()?.to_string();
    let attrs = item.get("attributes")?;

    let title = attrs.get("canonicalTitle")
        .and_then(|v| v.as_str()).unwrap_or("Sin título").to_string();

    let title_ja = attrs.get("titles")
        .and_then(|t| t.get("ja_jp")).and_then(|v| v.as_str()).map(str::to_string);

    let image = attrs.get("posterImage")
        .and_then(|p| p.get("medium").or_else(|| p.get("small")).or_else(|| p.get("original")))
        .and_then(|v| v.as_str()).unwrap_or("").to_string();

    let cover_image = attrs.get("coverImage")
        .and_then(|p| p.get("large").or_else(|| p.get("original")))
        .and_then(|v| v.as_str()).map(str::to_string);

    let synopsis = attrs.get("synopsis").or_else(|| attrs.get("description"))
        .and_then(|v| v.as_str()).unwrap_or("").to_string();

    let anime_type = attrs.get("subtype").or_else(|| attrs.get("showType"))
        .and_then(|v| v.as_str()).unwrap_or("TV").to_string();

    let status = attrs.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let episode_count = attrs.get("episodeCount").and_then(|v| v.as_u64()).map(|n| n as u32);

    let rating = attrs.get("averageRating").and_then(|v| v.as_str()).map(str::to_string);

    let year = attrs.get("startDate").and_then(|v| v.as_str())
        .and_then(|s| s.split('-').next()).and_then(|y| y.parse().ok());

    Some(KitsuAnime {
        id, title, title_ja, image, cover_image,
        synopsis, anime_type, status,
        episode_count, rating, year,
        genres: vec![],
    })
}

// ── Comandos Tauri ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn kitsu_browse(sort: Option<String>, page: Option<u32>) -> Result<String, String> {
    let page   = page.unwrap_or(1).max(1);
    let limit  = 20u32;
    let offset = (page - 1) * limit;

    let sort_param = match sort.as_deref().unwrap_or("popular") {
        "rating" => "-averageRating",
        "newest" => "-createdAt",
        "title"  => "canonicalTitle",
        _        => "-userCount",
    };

    let url = format!(
        "{}/anime?sort={}&page[limit]={}&page[offset]={}&fields[anime]=canonicalTitle,titles,posterImage,coverImage,subtype,showType,status,episodeCount,averageRating,startDate,synopsis",
        BASE, sort_param, limit, offset
    );
    let json = kitsu_get_cached(&url)?;

    let data = json.get("data").and_then(|d| d.as_array())
        .ok_or("Sin data en browse")?;
    let items: Vec<KitsuAnime> = data.iter().filter_map(parse_anime).collect();
    let total = json.get("meta").and_then(|m| m.get("count")).and_then(|v| v.as_u64()).unwrap_or(0);

    serde_json::to_string(&KitsuListResult {
        has_more: (offset as u64 + limit as u64) < total,
        total, items,
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kitsu_trending() -> Result<String, String> {
    let url  = format!("{}/trending/anime?limit=20", BASE);
    let json = kitsu_get_cached(&url)?;
    let data = json.get("data").and_then(|d| d.as_array()).ok_or("Sin data en trending")?;
    let items: Vec<KitsuAnime> = data.iter().filter_map(parse_anime).collect();
    serde_json::to_string(&KitsuListResult {
        has_more: false, total: items.len() as u64, items,
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kitsu_search(query: String) -> Result<String, String> {
    // filter[text] también necesita brackets literales
    let url = format!(
        "{}/anime?filter[text]={}&page[limit]=20&fields[anime]=canonicalTitle,titles,posterImage,subtype,showType,status,episodeCount,averageRating,startDate,synopsis",
        BASE,
        urlencoding::encode(&query)
    );
    let json = kitsu_get_cached(&url)?;
    let data = json.get("data").and_then(|d| d.as_array()).ok_or("Sin resultados")?;
    let items: Vec<KitsuAnime> = data.iter().filter_map(parse_anime).collect();
    serde_json::to_string(&KitsuListResult {
        has_more: false, total: items.len() as u64, items,
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kitsu_detail(id: String) -> Result<String, String> {
    let url = format!(
        "{}/anime/{}?include=genres&fields[anime]=canonicalTitle,titles,posterImage,coverImage,subtype,showType,status,episodeCount,averageRating,startDate,endDate,synopsis",
        BASE, id
    );
    let json = kitsu_get_cached(&url)?;

    let item = json.get("data").ok_or("Sin datos de detalle")?;
    let mut anime = parse_anime(item).ok_or("No se pudo parsear el anime")?;

    if let Some(included) = json.get("included").and_then(|v| v.as_array()) {
        anime.genres = included.iter()
            .filter(|x| x.get("type").and_then(|t| t.as_str()) == Some("genres"))
            .filter_map(|x| x.get("attributes").and_then(|a| a.get("name")).and_then(|n| n.as_str()).map(str::to_string))
            .collect();
    }

    serde_json::to_string(&anime).map_err(|e| e.to_string())
}

fn parse_episodes_page(json: &serde_json::Value) -> Vec<KitsuEpisode> {
    let Some(data) = json.get("data").and_then(|d| d.as_array()) else { return vec![] };
    data.iter().filter_map(|ep| {
        let id     = ep.get("id")?.as_str()?.to_string();
        let attrs  = ep.get("attributes")?;
        let number = attrs.get("number")?.as_u64()? as u32;
        let title      = attrs.get("canonicalTitle").and_then(|v| v.as_str()).map(str::to_string);
        let airdate    = attrs.get("airdate").and_then(|v| v.as_str()).map(str::to_string);
        let length_min = attrs.get("length").and_then(|v| v.as_u64()).map(|n| n as u32);
        let thumbnail  = attrs.get("thumbnail")
            .and_then(|t| t.get("original")).and_then(|v| v.as_str()).map(str::to_string);
        Some(KitsuEpisode { id, number, title, airdate, length_min, thumbnail })
    }).collect()
}

#[tauri::command]
pub async fn kitsu_episodes(anime_id: String) -> Result<String, String> {
    let limit = 20u32;

    // Página 0: obtener total
    let id0 = anime_id.clone();
    let url0 = format!(
        "{}/anime/{}/episodes?sort=number&page[limit]={}&page[offset]=0&fields[episodes]=number,canonicalTitle,airdate,length,thumbnail",
        BASE, id0, limit
    );
    let json0 = tokio::task::spawn_blocking(move || kitsu_get(&url0))
        .await.map_err(|e| e.to_string())??;

    let total = json0.get("meta").and_then(|m| m.get("count")).and_then(|v| v.as_u64()).unwrap_or(0);
    let mut all = parse_episodes_page(&json0);

    // Páginas restantes en paralelo
    if total > limit as u64 {
        let num_pages = ((total as u32).saturating_sub(limit) + limit - 1) / limit;
        let mut set = tokio::task::JoinSet::new();

        for p in 1..=num_pages {
            let id = anime_id.clone();
            set.spawn_blocking(move || {
                let url = format!(
                    "{}/anime/{}/episodes?sort=number&page[limit]={}&page[offset]={}&fields[episodes]=number,canonicalTitle,airdate,length,thumbnail",
                    BASE, id, limit, p * limit
                );
                kitsu_get(&url).map(|j| parse_episodes_page(&j))
            });
        }

        while let Some(res) = set.join_next().await {
            if let Ok(Ok(eps)) = res { all.extend(eps); }
        }
    }

    all.sort_by(|a, b| b.number.cmp(&a.number));

    #[derive(Serialize)]
    struct EpsResult { episodes: Vec<KitsuEpisode>, total: u64 }
    let total = all.len() as u64;
    serde_json::to_string(&EpsResult { episodes: all, total }).map_err(|e| e.to_string())
}
