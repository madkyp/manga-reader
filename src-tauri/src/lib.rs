mod scraper;
mod downloader;
mod vpn;
mod torrent;

use serde::Serialize;
use scraper::olympus::MangaItem;
use scraper::animeflv::AnimeItem;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[derive(Serialize)]
pub struct HomeUpdates {
    pub olympus:      Vec<MangaItem>,
    pub cerberus:     Vec<MangaItem>,
    pub taurus:       Vec<MangaItem>,
    pub leercapitulo: Vec<MangaItem>,
    pub animeflv:     Vec<AnimeItem>,
}

#[tauri::command]
async fn get_home_updates() -> HomeUpdates {
    let empty = || scraper::olympus::ListResult { results: vec![], has_more: false };
    tokio::task::spawn_blocking(move || {
        // Las 6 fuentes son HTTP independientes — ejecutar en paralelo
        let (oly1, oly2, cer, tau, leer, flv) = std::thread::scope(|s| {
            let t1  = s.spawn(|| scraper::olympus::get_latest(Some(1)).unwrap_or_else(|_| empty()));
            let t2  = s.spawn(|| scraper::olympus::get_latest(Some(2)).unwrap_or_else(|_| empty()));
            let t3  = s.spawn(|| scraper::cerberus::cerberus_latest(Some(1)).unwrap_or_else(|_| empty()));
            let t4  = s.spawn(|| scraper::taurus::taurus_latest(Some(1)).unwrap_or_else(|_| empty()));
            let t5  = s.spawn(|| scraper::leercapitulo::leercapitulo_latest(Some(1)).unwrap_or_else(|_| empty()));
            let t6  = s.spawn(|| scraper::animeflv::fetch_latest_items().unwrap_or_default());
            (t1.join().unwrap_or_else(|_| empty()),
             t2.join().unwrap_or_else(|_| empty()),
             t3.join().unwrap_or_else(|_| empty()),
             t4.join().unwrap_or_else(|_| empty()),
             t5.join().unwrap_or_else(|_| empty()),
             t6.join().unwrap_or_default())
        });

        let mut seen = std::collections::HashSet::new();
        let olympus: Vec<_> = oly1.results.into_iter().chain(oly2.results)
            .filter(|item| seen.insert(item.id.clone()))
            .collect();

        HomeUpdates {
            olympus,
            cerberus:     cer.results,
            taurus:       tau.results,
            leercapitulo: leer.results,
            animeflv:     flv,
        }
    })
    .await
    .unwrap_or_else(|_| HomeUpdates {
        olympus: vec![], cerberus: vec![], taurus: vec![], leercapitulo: vec![], animeflv: vec![],
    })
}

#[tauri::command]
fn health_check() -> String {
    "OK".to_string()
}

#[tauri::command]
async fn anime_ping() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://www4.animeflv.net/")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    Ok(format!("status={} bytes={}", status, text.len()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            // Guardar el resource_dir real para que ff_bin() pueda encontrar ffmpeg
            if let Ok(dir) = app.path().resource_dir() {
                eprintln!("[setup] resource_dir = {}", dir.display());
                torrent::RESOURCE_DIR.set(dir).ok();
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .level_for("librqbit", log::LevelFilter::Off)
                        .level_for("librqbit_dht", log::LevelFilter::Off)
                        .level_for("librqbit_peer_protocol", log::LevelFilter::Off)
                        .level_for("librqbit_utp", log::LevelFilter::Off)
                        .level_for("librqbit_tracker_comms", log::LevelFilter::Off)
                        .level_for("tracing::span", log::LevelFilter::Off)
                        .build(),
                )?;
            }

            // ── Tray icon ────────────────────────────────────────────────────
            let show = MenuItemBuilder::with_id("show", "Mostrar").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Salir").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("The Foundry APP")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        // Intercepta el cierre de ventana: oculta en vez de cerrar
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    window.hide().unwrap();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            anime_ping,
            get_home_updates,
            // OlympusScans
            scraper::olympus::get_latest,
            scraper::olympus::get_browse,
            scraper::olympus::get_info,
            scraper::olympus::get_pages,
            scraper::olympus::olympus_search,
            scraper::olympus::olympus_genres,
            scraper::olympus::olympus_filter_genres,
            // CerberusScans
            scraper::cerberus::cerberus_latest,
            scraper::cerberus::cerberus_browse,
            scraper::cerberus::cerberus_info,
            scraper::cerberus::cerberus_pages,
            scraper::cerberus::cerberus_search,
            scraper::cerberus::cerberus_genres,
            scraper::cerberus::cerberus_filter_genres,
            // TaurusScan
            scraper::taurus::taurus_latest,
            scraper::taurus::taurus_browse,
            scraper::taurus::taurus_info,
            scraper::taurus::taurus_pages,
            scraper::taurus::taurus_search,
            scraper::taurus::taurus_genres,
            scraper::taurus::taurus_filter_genres,
            // LeerCapitulo
            scraper::leercapitulo::leercapitulo_latest,
            scraper::leercapitulo::leercapitulo_browse,
            scraper::leercapitulo::leercapitulo_info,
            scraper::leercapitulo::leercapitulo_pages,
            scraper::leercapitulo::leercapitulo_search,
            // Descargas
            downloader::download_chapter,
            // VPN
            vpn::vpn_status,
            vpn::vpn_connect,
            vpn::vpn_disconnect,
            // Kitsu.io
            scraper::kitsu::kitsu_browse,
            scraper::kitsu::kitsu_trending,
            scraper::kitsu::kitsu_search,
            scraper::kitsu::kitsu_detail,
            scraper::kitsu::kitsu_episodes,
            // Nyaa.si torrents
            scraper::nyaa::nyaa_search,
            scraper::nyaa::nyaa_torrent_info,
            // Torrent streaming
            torrent::torrent_start,
            torrent::torrent_status,
            torrent::torrent_remove,
            torrent::torrent_open_external,
            torrent::torrent_transmux,
            // AnimeFLV
            scraper::animeflv::animeflv_latest,
            scraper::animeflv::animeflv_browse,
            scraper::animeflv::animeflv_search,
            scraper::animeflv::animeflv_detail,
            scraper::animeflv::animeflv_streams,
            scraper::animeflv::animeflv_image,
            scraper::animeflv::animeflv_extract,
            scraper::animeflv::anilist_episode_dates,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicación Tauri");
}
