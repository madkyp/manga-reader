/// Controla mpv (binario del sistema) a través de su socket IPC JSON.
/// En X11/Windows: mpv se incrusta en la ventana Tauri via --wid.
/// En Wayland: mpv abre ventana propia (externa).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

const SOCKET: &str = "/tmp/foundry-mpv.sock";

static WRITER: OnceLock<Mutex<Option<UnixStream>>> = OnceLock::new();
static PID:    OnceLock<Mutex<Option<u32>>>        = OnceLock::new();

fn writer() -> &'static Mutex<Option<UnixStream>> {
    WRITER.get_or_init(|| Mutex::new(None))
}
fn pid_slot() -> &'static Mutex<Option<u32>> {
    PID.get_or_init(|| Mutex::new(None))
}

// ── Busca el binario de mpv (bundle > junto al exe > PATH) ───────────────────
fn mpv_bin() -> std::path::PathBuf {
    let fname = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    if let Some(dir) = crate::torrent::RESOURCE_DIR.get() {
        let p = dir.join(fname);
        if p.exists() { return p; }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for p in [dir.join(fname), dir.join("resources").join(fname)] {
                if p.exists() { return p; }
            }
        }
    }
    std::path::PathBuf::from(fname)
}

// ── Detección de plataforma y WID ────────────────────────────────────────────

/// Devuelve el XID/HWND de la ventana si el entorno soporta embedding.
/// Devuelve None en Wayland o plataformas no soportadas.
fn get_embed_wid(window: &tauri::WebviewWindow) -> Option<u64> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // En Wayland no hay embedding de ventanas nativas
    #[cfg(target_os = "linux")]
    if is_wayland() {
        return None;
    }

    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xlib(h)  => Some(h.window as u64),       // c_ulong
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xcb(h)   => Some(h.window.get() as u64), // NonZeroU32
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as u64),   // NonZero<isize>
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("wayland") => true,
        _ => std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err(),
    }
}

// ── Baja la ventana hijo de mpv al fondo del z-order (X11) ──────────────────
/// Después de que mpv crea su ventana hijo dentro de parent_xid,
/// la bajamos debajo del WebKit para que el overlay transparente funcione.
#[cfg(target_os = "linux")]
fn lower_mpv_child(parent_xid: u64) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt, StackMode};

    let Ok((conn, _)) = x11rb::rust_connection::RustConnection::connect(None) else { return };

    // query_tree devuelve hijos en orden bottom-to-top; el último es el más reciente (mpv)
    let Ok(cookie) = conn.query_tree(parent_xid as u32) else { return };
    let Ok(reply)  = cookie.reply() else { return };

    if let Some(&top) = reply.children.last() {
        // StackMode::Below sin sibling baja el hijo al fondo del stack
        let _ = conn.configure_window(top, &ConfigureWindowAux::new().stack_mode(StackMode::BELOW));
        let _ = conn.flush();
    }
}

// ── Envío de comandos IPC ────────────────────────────────────────────────────
fn send(args: serde_json::Value) -> Result<(), String> {
    let mut guard = writer().lock().unwrap();
    let stream = guard.as_mut().ok_or("mpv no está activo")?;
    let mut msg = serde_json::to_string(&serde_json::json!({ "command": args }))
        .map_err(|e| e.to_string())?;
    msg.push('\n');
    stream.write_all(msg.as_bytes()).map_err(|e| e.to_string())
}

// ── Comando principal ────────────────────────────────────────────────────────

/// Abre (o reemplaza) la reproducción.
/// Devuelve `true` si el vídeo se incrustra en la ventana Tauri (X11/Windows),
/// `false` si mpv abre ventana propia (Wayland/macOS).
#[tauri::command]
pub fn mpv_open(path: String, window: tauri::WebviewWindow, app: AppHandle) -> Result<bool, String> {
    mpv_close().ok();
    let _ = std::fs::remove_file(SOCKET);

    let embed_wid = get_embed_wid(&window);
    let embedded  = embed_wid.is_some();

    let mut args: Vec<String> = vec![
        path,
        format!("--input-ipc-server={}", SOCKET),
        "--keep-open=yes".into(),
        "--sub-auto=fuzzy".into(),
        "--input-default-bindings=yes".into(),
        "--volume=100".into(),
    ];

    if let Some(wid) = embed_wid {
        args.push(format!("--wid={}", wid));
        args.push("--force-window=no".into());
        args.push("--osc=no".into());
        args.push("--osd-level=0".into());
    } else {
        args.push("--force-window=yes".into());
        args.push("--osc=yes".into());
    }

    let child = std::process::Command::new(mpv_bin())
        .args(&args)
        .spawn()
        .map_err(|e| format!("No se pudo lanzar mpv: {}. ¿Está instalado?", e))?;

    *pid_slot().lock().unwrap() = Some(child.id());

    // Hilo: espera socket, conecta, reenvía eventos
    std::thread::spawn(move || {
        // Esperar hasta 5 s a que mpv cree el socket
        let mut connected = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if std::path::Path::new(SOCKET).exists() {
                connected = true;
                break;
            }
        }
        if !connected {
            let _ = app.emit("mpv://error", "mpv no creó el socket IPC");
            return;
        }

        let stream = match UnixStream::connect(SOCKET) {
            Ok(s) => s,
            Err(e) => { let _ = app.emit("mpv://error", format!("Socket: {}", e)); return; }
        };
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };
        *writer().lock().unwrap() = Some(stream);

        // En X11: bajar la ventana hijo de mpv al fondo (detrás del WebKit)
        #[cfg(target_os = "linux")]
        if let Some(wid) = embed_wid {
            std::thread::sleep(std::time::Duration::from_millis(150));
            lower_mpv_child(wid);
        }

        let _ = send(serde_json::json!(["observe_property", 1, "time-pos"]));
        let _ = send(serde_json::json!(["observe_property", 2, "duration"]));
        let _ = send(serde_json::json!(["observe_property", 3, "pause"]));
        let _ = app.emit("mpv://time-pos", 0f64);

        let reader = BufReader::new(reader_stream);
        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let val: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v, Err(_) => continue,
            };

            if val.get("event").and_then(|v| v.as_str()) == Some("property-change") {
                match val["name"].as_str() {
                    Some("time-pos") => { let _ = app.emit("mpv://time-pos", val["data"].as_f64().unwrap_or(0.0)); }
                    Some("duration") => { let _ = app.emit("mpv://duration", val["data"].as_f64().unwrap_or(0.0)); }
                    Some("pause")    => { let _ = app.emit("mpv://pause",    val["data"].as_bool().unwrap_or(false)); }
                    _ => {}
                }
            } else if matches!(
                val.get("event").and_then(|v| v.as_str()),
                Some("end-file") | Some("shutdown")
            ) {
                let _ = app.emit("mpv://eof", ());
                break;
            }
        }

        *writer().lock().unwrap() = None;
        *pid_slot().lock().unwrap() = None;
        let _ = std::fs::remove_file(SOCKET);
    });

    Ok(embedded)
}

// ── Controles de reproducción ────────────────────────────────────────────────

#[tauri::command]
pub fn mpv_pause_toggle() -> Result<(), String> {
    send(serde_json::json!(["cycle", "pause"]))
}

#[tauri::command]
pub fn mpv_seek(pos: f64) -> Result<(), String> {
    send(serde_json::json!(["seek", pos, "absolute"]))
}

#[tauri::command]
pub fn mpv_set_volume(vol: i64) -> Result<(), String> {
    send(serde_json::json!(["set_property", "volume", vol]))
}

#[tauri::command]
pub fn mpv_close() -> Result<(), String> {
    send(serde_json::json!(["quit"])).ok();
    *writer().lock().unwrap() = None;
    if let Some(pid) = pid_slot().lock().unwrap().take() {
        libc_kill(pid);
    }
    let _ = std::fs::remove_file(SOCKET);
    Ok(())
}

#[tauri::command]
pub fn mpv_raw_command(cmd: String, args: Vec<String>) -> Result<(), String> {
    let mut arr: Vec<serde_json::Value> = vec![serde_json::Value::String(cmd)];
    arr.extend(args.into_iter().map(serde_json::Value::String));
    send(serde_json::Value::Array(arr))
}

// ── Pistas de subtítulos ─────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct SubTrack {
    pub id:       i64,
    pub lang:     String,
    pub title:    String,
    pub selected: bool,
    pub external: bool,
}

fn query(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut stream = UnixStream::connect(SOCKET)
        .map_err(|_| "mpv no está activo".to_string())?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();

    let req = serde_json::json!({ "command": args, "request_id": 1 });
    let mut msg = serde_json::to_string(&req).unwrap();
    msg.push('\n');
    stream.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;

    let reader = BufReader::new(stream);
    for line in reader.lines().take(200) {
        let line = match line { Ok(l) => l, Err(_) => break };
        let val: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        if val.get("request_id").and_then(|v| v.as_u64()) == Some(1) {
            return if val["error"].as_str() == Some("success") {
                Ok(val["data"].clone())
            } else {
                Err(val["error"].as_str().unwrap_or("error mpv").to_string())
            };
        }
    }
    Err("Sin respuesta de mpv".to_string())
}

#[tauri::command]
pub fn mpv_get_tracks() -> Result<Vec<SubTrack>, String> {
    let data = query(serde_json::json!(["get_property", "track-list"]))?;
    let arr  = data.as_array().ok_or("track-list no es array")?;

    Ok(arr.iter()
        .filter(|t| t["type"].as_str() == Some("sub"))
        .map(|t| SubTrack {
            id:       t["id"].as_i64().unwrap_or(0),
            lang:     t.get("lang").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title:    t.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            selected: t.get("selected").and_then(|v| v.as_bool()).unwrap_or(false),
            external: t.get("external").and_then(|v| v.as_bool()).unwrap_or(false),
        })
        .collect())
}

#[tauri::command]
pub fn mpv_set_sub(id: i64) -> Result<(), String> {
    if id == 0 {
        send(serde_json::json!(["set_property", "sub-visibility", false]))
    } else {
        send(serde_json::json!(["set_property", "sid", id]))?;
        send(serde_json::json!(["set_property", "sub-visibility", true]))
    }
}

// ── Kill por PID ─────────────────────────────────────────────────────────────
#[cfg(unix)]
fn libc_kill(pid: u32) {
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM); }
}
#[cfg(not(unix))]
fn libc_kill(_pid: u32) {}
