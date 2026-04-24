// Torrent streaming — librqbit descarga a disco + sirve HTTP con Range support.
// El frontend usa la URL http://127.0.0.1:PORT/torrents/{id}/stream/{file_idx}
// para que el <video> pida solo los bytes que necesita (streaming real).
//
// Además, un segundo servidor HTTP (transmux) usa ffmpeg para convertir MKV→MP4
// al vuelo y extraer pistas de subtítulos, para reproducción in-app.
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::{Arc, OnceLock}};
use tokio::{net::TcpListener, sync::Mutex};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse,
    Session, SessionOptions,
    api::{Api, TorrentIdOrHash},
    http_api::{HttpApi, HttpApiOptions},
};

// Inicializado en lib.rs setup() con app.path().resource_dir()
pub static RESOURCE_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

fn ff_bin(name: &str) -> std::path::PathBuf {
    let fname = if cfg!(windows) { format!("{}.exe", name) } else { name.to_string() };
    // 1. resource_dir resuelto por Tauri en startup (más fiable)
    if let Some(dir) = RESOURCE_DIR.get() {
        let c = dir.join(&fname);
        if c.exists() { return c; }
    }
    // 2. Junto al exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in [dir.join(&fname), dir.join("resources").join(&fname)] {
                if c.exists() { return c; }
            }
        }
    }
    std::path::PathBuf::from(name)
}

fn cmd_sync(prog: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(prog);
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x08000000); }
    c
}

fn cmd_async(prog: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut c = tokio::process::Command::new(prog);
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x08000000); }
    c
}

static SESSION: OnceLock<std::sync::Arc<Session>> = OnceLock::new();
static API_PORT: OnceLock<u16> = OnceLock::new();
static INIT_LOCK: Mutex<bool> = Mutex::const_new(false);

static TRANSMUX_PORT: OnceLock<u16> = OnceLock::new();

const BITMAP_CODECS: &[&str] = &["hdmv_pgs_subtitle", "dvd_subtitle", "dvbsub", "pgssub", "xsub"];
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "webm", "mov"];
static TRANSMUX_SOURCES: OnceLock<Arc<Mutex<HashMap<String, TransmuxSource>>>> = OnceLock::new();

#[derive(Clone)]
struct TransmuxSource {
    url:  String,          // HTTP stream de librqbit (para h_video)
    path: Option<PathBuf>, // Ruta en disco (para h_sub — más fiable que HTTP)
}

fn transmux_sources() -> Arc<Mutex<HashMap<String, TransmuxSource>>> {
    TRANSMUX_SOURCES
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

fn download_dir() -> PathBuf {
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    base.join(".local/share/foundry/torrents")
}

async fn ensure_session() -> Result<std::sync::Arc<Session>, String> {
    if let Some(s) = SESSION.get() {
        return Ok(s.clone());
    }
    let _guard = INIT_LOCK.lock().await;
    if let Some(s) = SESSION.get() {
        return Ok(s.clone());
    }
    let dir = download_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let session = Session::new_with_opts(
        dir,
        SessionOptions { disable_dht: false, ..Default::default() },
    )
    .await
    .map_err(|e| e.to_string())?;

    // Levantar HTTP API en puerto aleatorio (localhost only) para streaming con Range.
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let api = Api::new(session.clone(), None, None);
    let http_api = HttpApi::new(api, Some(HttpApiOptions { read_only: false, basic_auth: None }));

    tokio::spawn(async move {
        let _ = http_api.make_http_api_and_run(listener, None).await;
    });

    API_PORT.set(port).ok();
    SESSION.set(session).ok();
    Ok(SESSION.get().unwrap().clone())
}

#[derive(Serialize, Clone)]
pub struct TorrentStartResult {
    pub torrent_id: usize,
    pub files:      Vec<TorrentFile>,
    pub name:       String,
    pub output_dir: String,
}

#[derive(Serialize, Clone)]
pub struct TorrentFile {
    pub index:      usize,
    pub name:       String,
    pub rel_path:   String,
    pub abs_path:   String,
    pub stream_url: String,   // http://127.0.0.1:PORT/torrents/{id}/stream/{index}
    pub size:       u64,
    pub is_video:   bool,
}

#[derive(Serialize, Clone)]
pub struct TorrentStatus {
    pub torrent_id:   usize,
    pub downloaded:   u64,
    pub total:        u64,
    pub progress_pct: f32,
    pub speed_bps:    u64,
    pub peers:        u32,
    pub state:        String,
    pub finished:     bool,
}

#[tauri::command]
pub async fn torrent_start(url: String) -> Result<TorrentStartResult, String> {
    let session = ensure_session().await?;
    let port    = *API_PORT.get().ok_or("API port no inicializado")?;
    let dir     = download_dir();

    let opts = AddTorrentOptions {
        output_folder: Some(dir.to_string_lossy().to_string()),
        overwrite: true,  // Permite reusar archivos de sesiones previas
        ..Default::default()
    };

    let response = session
        .add_torrent(AddTorrent::from_url(&url), Some(opts))
        .await
        .map_err(|e| e.to_string())?;

    let (torrent_id, handle) = match response {
        AddTorrentResponse::Added(id, h)          => (id, h),
        AddTorrentResponse::AlreadyManaged(id, h) => (id, h),
        AddTorrentResponse::ListOnly(_)           => return Err("Torrent list-only".into()),
    };

    // Esperar metadatos (magnets sin info). 60s máximo.
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        handle.wait_until_initialized(),
    ).await;

    let name = handle.name().unwrap_or_else(|| {
        handle.shared().info_hash.as_string()
    });

    // Si tras el timeout no hay metadatos, devolver error claro.
    if handle.with_metadata(|_| ()).is_err() {
        return Err("No se pudieron obtener los metadatos del torrent (sin peers o magnet inválido). Prueba otro torrent.".into());
    }

    let files = handle
        .with_metadata(|m| -> Vec<TorrentFile> {
            m.file_infos
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let rel = f.relative_filename.to_string_lossy().to_string();
                    let abs = dir.join(&f.relative_filename);
                    let ext = rel.rsplit('.').next().unwrap_or("").to_lowercase();
                    let fname = f
                        .relative_filename
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| rel.clone());
                    TorrentFile {
                        index:      i,
                        name:       fname,
                        rel_path:   rel,
                        abs_path:   abs.to_string_lossy().to_string(),
                        stream_url: format!("http://127.0.0.1:{}/torrents/{}/stream/{}", port, torrent_id, i),
                        size:       f.len,
                        is_video:   VIDEO_EXTENSIONS.contains(&ext.as_str()),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(TorrentStartResult {
        torrent_id,
        files,
        name: name.clone(),
        output_dir: dir.join(name).to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn torrent_status(torrent_id: usize) -> Result<TorrentStatus, String> {
    let session = SESSION.get().ok_or("Sesión no iniciada")?;
    let handle  = session
        .get(TorrentIdOrHash::Id(torrent_id))
        .ok_or("Torrent no encontrado")?;

    let stats   = handle.stats();
    let speed   = stats.live.as_ref()
        .map(|l| (l.download_speed.mbps * 125_000.0) as u64)
        .unwrap_or(0);
    let peers   = stats.live.as_ref()
        .map(|l| l.snapshot.peer_stats.live as u32)
        .unwrap_or(0);

    Ok(TorrentStatus {
        torrent_id,
        downloaded:   stats.progress_bytes,
        total:        stats.total_bytes,
        progress_pct: if stats.total_bytes > 0 {
            stats.progress_bytes as f32 / stats.total_bytes as f32 * 100.0
        } else { 0.0 },
        speed_bps:    speed,
        peers,
        state:        format!("{}", stats.state),
        finished:     stats.finished,
    })
}

// ── Transmux (ffmpeg MKV → fMP4 + extract subs) ──────────────────────────────

#[derive(Serialize, Clone)]
pub struct SubtitleTrack {
    pub index: u32,
    pub lang:  String,
    pub title: String,
    pub url:   String,
}

#[derive(Serialize)]
pub struct TransmuxResult {
    pub video_url: String,
    pub subs:      Vec<SubtitleTrack>,
}

async fn ensure_transmux_server() -> Result<u16, String> {
    use axum::{Router, routing::get, extract::Path, response::{Response, IntoResponse}, body::Body, http::StatusCode};
    use tokio_util::io::ReaderStream;

    if let Some(&p) = TRANSMUX_PORT.get() { return Ok(p); }

    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    async fn h_video(Path(id): Path<String>) -> Result<Response, StatusCode> {
        let src = transmux_sources().lock().await.get(&id).map(|s| s.url.clone()).ok_or(StatusCode::NOT_FOUND)?;
        let mut child = cmd_async(ff_bin("ffmpeg"))
            .args([
                "-hide_banner",
                "-loglevel", "info",
                "-fflags", "+genpts+nobuffer",
                "-analyzeduration", "10M",
                "-probesize", "10M",
                "-i", &src,
                "-map", "0:v:0",
                "-map", "0:a:0?",
                "-c:v", "copy",
                "-c:a", "aac", "-b:a", "160k", "-ac", "2",
                "-movflags", "+frag_keyframe+empty_moov+default_base_moof",
                "-frag_duration", "2000000",
                "-f", "mp4",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut stdout = child.stdout.take().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        // Duplex pipe: mantiene child vivo en la tarea hasta que ffmpeg termina.
        // Sin esto, child se dropea al salir del handler y kill_on_drop mata ffmpeg.
        let (mut writer, reader) = tokio::io::duplex(256 * 1024);
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stdout, &mut writer).await;
            drop(child);
        });
        let stream = ReaderStream::new(reader);
        Ok((
            [
                ("Content-Type", "video/mp4"),
                ("Access-Control-Allow-Origin", "*"),
                ("Cache-Control", "no-store"),
            ],
            Body::from_stream(stream),
        ).into_response())
    }

    async fn h_sub(Path((id, idx)): Path<(String, u32)>) -> Result<Response, StatusCode> {
        let sources = transmux_sources();
        let map = sources.lock().await;
        let src = map.get(&id).cloned().ok_or(StatusCode::NOT_FOUND)?;
        drop(map);
        // Preferir ruta en disco: evita competencia con el stream HTTP del video
        let input = src.path
            .as_ref()
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(src.url);

        // Subtítulos son pequeños (<100 KB) → .output() es más fiable que streaming
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            cmd_async(ff_bin("ffmpeg"))
                .args([
                    "-loglevel", "error",
                    "-i", &input,
                    "-map", &format!("0:s:{}", idx),
                    "-f", "webvtt",
                    "pipe:1",
                ])
                .output(),
        )
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if out.stdout.is_empty() { return Err(StatusCode::NO_CONTENT); }

        Ok((
            [
                ("Content-Type", "text/vtt; charset=utf-8"),
                ("Access-Control-Allow-Origin", "*"),
            ],
            out.stdout,
        ).into_response())
    }

    let app = Router::new()
        .route("/transmux/{id}/video", get(h_video))
        .route("/transmux/{id}/sub/{idx}", get(h_sub));

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    TRANSMUX_PORT.set(port).ok();
    Ok(port)
}

/// Arranca un transmux (ffmpeg) para un archivo dentro de un torrent activo.
/// Devuelve una URL MP4 reproducible + lista de pistas de subtítulos embebidos.
#[tauri::command]
pub async fn torrent_transmux(torrent_id: usize, file_idx: usize) -> Result<TransmuxResult, String> {
    // Verificar que ffmpeg + ffprobe estén disponibles
    for name in ["ffmpeg", "ffprobe"] {
        let path = ff_bin(name);
        let exists = path.exists();
        let ok = exists && cmd_sync(&path)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok();
        if !ok {
            return Err(format!(
                "{} no encontrado.\nBuscado en: {}\nExiste: {}\nRESOURCE_DIR: {}",
                name,
                path.display(),
                exists,
                RESOURCE_DIR.get().map(|p| p.display().to_string()).unwrap_or_else(|| "no inicializado".into())
            ));
        }
    }

    let api_port      = *API_PORT.get().ok_or("Sesión torrent no iniciada")?;
    let transmux_port = ensure_transmux_server().await?;

    let source_url = format!("http://127.0.0.1:{}/torrents/{}/stream/{}", api_port, torrent_id, file_idx);
    let session_id = format!("{}-{}", torrent_id, file_idx);

    // Ruta en disco para subtitle extraction (más fiable que HTTP)
    let file_path = SESSION.get()
        .and_then(|s| s.get(TorrentIdOrHash::Id(torrent_id)))
        .and_then(|h| h.with_metadata(|m| {
            m.file_infos.get(file_idx).map(|f| download_dir().join(&f.relative_filename))
        }).ok().flatten());

    transmux_sources().lock().await.insert(session_id.clone(), TransmuxSource {
        url: source_url.clone(),
        path: file_path.clone(),
    });

    let probe_input = file_path
        .as_ref()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(source_url.clone());

    let subs = probe_subtitles(&probe_input, &session_id, transmux_port).await.unwrap_or_default();

    Ok(TransmuxResult {
        video_url: format!("http://127.0.0.1:{}/transmux/{}/video", transmux_port, session_id),
        subs,
    })
}

async fn probe_subtitles(source_url: &str, session_id: &str, port: u16) -> Result<Vec<SubtitleTrack>, String> {
    let source = source_url.to_string();
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cmd_async(ff_bin("ffprobe"))
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_streams",
                "-select_streams", "s",
                &source,
            ])
            .output(),
    )
    .await
    .map_err(|_| "ffprobe timeout")?
    .map_err(|e| format!("ffprobe: {}", e))?;

    if !out.status.success() {
        return Err(format!("ffprobe exit {}", out.status));
    }

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = json.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default();

    let mut subs = Vec::new();
    for (i, s) in streams.iter().enumerate() {
        let codec = s.get("codec_name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        if BITMAP_CODECS.iter().any(|&c| codec == c) { continue; }
        let tags  = s.get("tags");
        let lang  = tags.and_then(|t| t.get("language")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let title = tags.and_then(|t| t.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        subs.push(SubtitleTrack {
            index: i as u32,
            lang,
            title,
            url: format!("http://127.0.0.1:{}/transmux/{}/sub/{}", port, session_id, i),
        });
    }
    Ok(subs)
}

/// Abre una URL en un reproductor externo — mpv en Linux, VLC en Windows.
#[tauri::command]
pub fn torrent_open_external(url: String) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // Construir candidatos usando variables de entorno para no asumir C:\
        let mut candidates: Vec<String> = Vec::new();
        for var in &["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(pf) = std::env::var(var) {
                candidates.push(format!(r"{}\VideoLAN\VLC\vlc.exe", pf));
                candidates.push(format!(r"{}\mpv\mpv.exe", pf));
                candidates.push(format!(r"{}\mpv-x86_64\mpv.exe", pf));
            }
        }
        // LocalAppData (instalaciones de usuario)
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(format!(r"{}\Programs\VLC\vlc.exe", local));
        }
        // PATH (vlc.exe o mpv.exe en el PATH del sistema)
        candidates.push("vlc".to_string());
        candidates.push("mpv".to_string());

        for p in &candidates {
            if cmd_sync(p).arg(&url).spawn().is_ok() {
                let name = std::path::Path::new(p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(p);
                return Ok(name.to_string());
            }
        }

        // Fallback: abrir con el reproductor predeterminado de Windows
        return cmd_sync("cmd")
            .args(["/C", "start", "", url.as_str()])
            .spawn()
            .map(|_| "default".to_string())
            .map_err(|e| format!("No se encontró reproductor: {}", e));
    }

    #[cfg(target_os = "macos")]
    let players: &[&str] = &["mpv", "/Applications/VLC.app/Contents/MacOS/VLC", "vlc"];

    #[cfg(all(unix, not(target_os = "macos")))]
    let players: &[&str] = &["mpv", "vlc"];

    #[cfg(not(target_os = "windows"))]
    for p in players {
        if std::process::Command::new(p).arg(&url).spawn().is_ok() {
            let name = std::path::Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(p);
            return Ok(name.to_string());
        }
    }

    #[cfg(target_os = "macos")]
    return std::process::Command::new("open").arg(&url).spawn()
        .map(|_| "open".to_string())
        .map_err(|e| format!("No se encontró reproductor externo: {}", e));

    #[cfg(all(unix, not(target_os = "macos")))]
    return std::process::Command::new("xdg-open").arg(&url).spawn()
        .map(|_| "xdg-open".to_string())
        .map_err(|e| format!("No se encontró reproductor externo: {}", e));
}

#[tauri::command]
pub async fn torrent_remove(torrent_id: usize, delete_files: bool) -> Result<(), String> {
    let session = SESSION.get().ok_or("Sesión no iniciada")?;
    session
        .delete(TorrentIdOrHash::Id(torrent_id), delete_files)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
