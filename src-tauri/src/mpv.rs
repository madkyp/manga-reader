/// Paso 1: instancia libmpv con su ventana propia + IPC básica.
/// Abre archivos/URLs, emite eventos de tiempo/duración/pausa al frontend.

use libmpv2::{
    events::{Event, PropertyData},
    Format, Mpv,
};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

// Handle principal — compartido por los comandos (Mpv: Send + Sync, & para commands)
static MPV: OnceLock<Mutex<Option<Arc<Mpv>>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Arc<Mpv>>> {
    MPV.get_or_init(|| Mutex::new(None))
}

/// Abre (o reemplaza) la reproducción con el archivo/URL indicado.
/// En Wayland/Windows, libmpv gestiona su propia ventana nativa.
#[tauri::command]
pub fn mpv_open(path: String, app: AppHandle) -> Result<(), String> {
    let mut guard = slot().lock().unwrap();

    // Si ya hay instancia activa, simplemente carga el archivo
    if let Some(ref mpv) = *guard {
        return mpv
            .command("loadfile", &[path.as_str()])
            .map_err(|e| format!("{e:?}"));
    }

    // ── Crear nueva instancia ────────────────────────────────────────────────
    let mpv = Mpv::with_initializer(|init| {
        init.set_property("force-window", "yes")?;
        init.set_property("osc", "yes")?;
        init.set_property("keep-open", "yes")?;
        init.set_property("input-default-bindings", "yes")?;
        init.set_property("input-vo-keyboard", "yes")?;
        init.set_property("sub-auto", "fuzzy")?;
        init.set_property("volume", 100_i64)?;
        Ok(())
    })
    .map_err(|e| format!("mpv init: {e:?}"))?;

    // ── Registrar propiedades a observar ─────────────────────────────────────
    mpv.disable_deprecated_events()
        .map_err(|e| format!("disable_deprecated: {e:?}"))?;
    mpv.observe_property("time-pos", Format::Double, 0)
        .map_err(|e| format!("observe time-pos: {e:?}"))?;
    mpv.observe_property("duration", Format::Double, 1)
        .map_err(|e| format!("observe duration: {e:?}"))?;
    mpv.observe_property("pause", Format::Flag, 2)
        .map_err(|e| format!("observe pause: {e:?}"))?;

    // ── Cargar el archivo ────────────────────────────────────────────────────
    mpv.command("loadfile", &[path.as_str()])
        .map_err(|e| format!("loadfile: {e:?}"))?;

    // ── Hilo de eventos ──────────────────────────────────────────────────────
    // create_client crea un handle secundario que comparte el mismo contexto mpv.
    // Ese handle es el que usamos para wait_event(&mut self).
    let mut event_client = mpv
        .create_client(None)
        .map_err(|e| format!("create_client: {e:?}"))?;

    std::thread::spawn(move || {
        loop {
            match event_client.wait_event(1.0) {
                Some(Ok(Event::PropertyChange { name, change, .. })) => {
                    match (name, change) {
                        ("time-pos", PropertyData::Double(t)) => {
                            let _ = app.emit("mpv://time-pos", t);
                        }
                        ("duration", PropertyData::Double(d)) => {
                            let _ = app.emit("mpv://duration", d);
                        }
                        ("pause", PropertyData::Flag(p)) => {
                            let _ = app.emit("mpv://pause", p);
                        }
                        _ => {}
                    }
                }
                Some(Ok(Event::EndFile(_))) | Some(Ok(Event::Shutdown)) => {
                    let _ = app.emit("mpv://eof", ());
                    break;
                }
                Some(Err(e)) => {
                    eprintln!("[mpv] event error: {e:?}");
                    break;
                }
                None | Some(Ok(_)) => {}
            }
        }
        // Limpiar instancia cuando mpv termina
        if let Some(s) = MPV.get() {
            *s.lock().unwrap() = None;
        }
    });

    *guard = Some(Arc::new(mpv));
    Ok(())
}

/// Pausa / reanuda (toggle).
#[tauri::command]
pub fn mpv_pause_toggle() -> Result<(), String> {
    with_mpv(|m| m.command("cycle", &["pause"]))
}

/// Seek absoluto en segundos.
#[tauri::command]
pub fn mpv_seek(pos: f64) -> Result<(), String> {
    let s = format!("{pos:.3}");
    with_mpv(|m| m.command("seek", &[s.as_str(), "absolute"]))
}

/// Cambia el volumen (0–100).
#[tauri::command]
pub fn mpv_set_volume(vol: i64) -> Result<(), String> {
    with_mpv(|m| m.set_property("volume", vol))
}

/// Cierra el reproductor.
#[tauri::command]
pub fn mpv_close() -> Result<(), String> {
    with_mpv(|m| m.command("quit", &[]))
}

/// Comando mpv genérico (depuración / extensión futura).
#[tauri::command]
pub fn mpv_raw_command(cmd: String, args: Vec<String>) -> Result<(), String> {
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    with_mpv(|m| m.command(cmd.as_str(), &refs))
}

// ── Helper interno ────────────────────────────────────────────────────────────
fn with_mpv<F>(f: F) -> Result<(), String>
where
    F: FnOnce(&Mpv) -> Result<(), libmpv2::Error>,
{
    let guard = slot().lock().unwrap();
    let mpv = guard.as_ref().ok_or("mpv no está activo")?;
    f(mpv).map_err(|e| format!("{e:?}"))
}
