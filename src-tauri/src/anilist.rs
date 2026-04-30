// Integración con AniList — usa Personal Access Tokens (sin OAuth).
// El usuario genera su token en: anilist.co/settings/developer
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

const GRAPHQL_URL: &str = "https://graphql.anilist.co";

// ── Tipos ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct AniListManga {
    pub id:         i32,
    pub title:      String,
    pub cover:      String,
    pub status:     String,
    pub progress:   i32,   // capítulos leídos según AniList
    pub total_chs:  Option<i32>,
}

// ── Persistencia del token ────────────────────────────────────────────────────

fn token_path(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("anilist_token.txt")
}

#[tauri::command]
pub fn anilist_save_token(app: tauri::AppHandle, token: String) -> Result<(), String> {
    let path = token_path(&app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, token.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn anilist_get_token(app: tauri::AppHandle) -> Option<String> {
    let path = token_path(&app);
    std::fs::read_to_string(&path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[tauri::command]
pub fn anilist_clear_token(app: tauri::AppHandle) {
    let _ = std::fs::remove_file(token_path(&app));
}

// ── GraphQL helper ────────────────────────────────────────────────────────────

async fn gql(token: &str, query: &str, variables: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "query": query, "variables": variables });
    let resp = client
        .post(GRAPHQL_URL)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Error de red: {}", e))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(errors) = json.get("errors") {
        return Err(format!("AniList error: {}", errors));
    }
    Ok(json["data"].clone())
}

// ── Buscar manga por título ───────────────────────────────────────────────────

#[tauri::command]
pub async fn anilist_search(app: tauri::AppHandle, title: String) -> Result<Vec<AniListManga>, String> {
    let token = anilist_get_token(app).ok_or("No hay token de AniList configurado")?;
    let query = r#"
        query ($search: String) {
          Page(perPage: 10) {
            media(search: $search, type: MANGA) {
              id
              title { romaji english native }
              coverImage { medium }
              status
              chapters
              mediaListEntry { progress }
            }
          }
        }
    "#;
    let data = gql(&token, query, serde_json::json!({ "search": title })).await?;
    let items = data["Page"]["media"].as_array().cloned().unwrap_or_default();
    Ok(items.iter().map(|m| {
        let title = m["title"]["english"].as_str()
            .or_else(|| m["title"]["romaji"].as_str())
            .unwrap_or("").to_string();
        AniListManga {
            id:        m["id"].as_i64().unwrap_or(0) as i32,
            title,
            cover:     m["coverImage"]["medium"].as_str().unwrap_or("").to_string(),
            status:    m["status"].as_str().unwrap_or("").to_string(),
            progress:  m["mediaListEntry"]["progress"].as_i64().unwrap_or(0) as i32,
            total_chs: m["chapters"].as_i64().map(|n| n as i32),
        }
    }).collect())
}

// ── Actualizar progreso ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn anilist_update_progress(
    app: tauri::AppHandle,
    media_id: i32,
    progress: i32,
) -> Result<(), String> {
    let token = anilist_get_token(app).ok_or("No hay token de AniList configurado")?;
    let mutation = r#"
        mutation ($mediaId: Int, $progress: Int) {
          SaveMediaListEntry(mediaId: $mediaId, progress: $progress) {
            id progress
          }
        }
    "#;
    gql(&token, mutation, serde_json::json!({ "mediaId": media_id, "progress": progress })).await?;
    Ok(())
}

// ── Importar lista del usuario ────────────────────────────────────────────────

#[tauri::command]
pub async fn anilist_get_list(app: tauri::AppHandle) -> Result<Vec<AniListManga>, String> {
    let token = anilist_get_token(app.clone()).ok_or("No hay token de AniList configurado")?;

    // Obtener el ID del usuario autenticado
    let viewer_q = r#"query { Viewer { id } }"#;
    let viewer_data = gql(&token, viewer_q, serde_json::Value::Null).await?;
    let user_id = viewer_data["Viewer"]["id"].as_i64().ok_or("No se pudo obtener el ID de usuario")?;

    let query = r#"
        query ($userId: Int) {
          MediaListCollection(userId: $userId, type: MANGA) {
            lists {
              entries {
                progress
                media {
                  id
                  title { romaji english }
                  coverImage { medium }
                  status
                  chapters
                }
              }
            }
          }
        }
    "#;
    let data = gql(&token, query, serde_json::json!({ "userId": user_id })).await?;
    let lists = data["MediaListCollection"]["lists"].as_array().cloned().unwrap_or_default();
    let mut result = Vec::new();
    for list in &lists {
        let entries = list["entries"].as_array().cloned().unwrap_or_default();
        for entry in &entries {
            let m = &entry["media"];
            let title = m["title"]["english"].as_str()
                .or_else(|| m["title"]["romaji"].as_str())
                .unwrap_or("").to_string();
            result.push(AniListManga {
                id:        m["id"].as_i64().unwrap_or(0) as i32,
                title,
                cover:     m["coverImage"]["medium"].as_str().unwrap_or("").to_string(),
                status:    m["status"].as_str().unwrap_or("").to_string(),
                progress:  entry["progress"].as_i64().unwrap_or(0) as i32,
                total_chs: m["chapters"].as_i64().map(|n| n as i32),
            });
        }
    }
    Ok(result)
}
