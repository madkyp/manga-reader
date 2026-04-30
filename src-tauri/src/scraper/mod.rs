// Submódulos de scraping, uno por fuente de manga
pub mod olympus;      // OlympusScans — olympusbiblioteca.com
pub mod cerberus;     // CerberusScans — legionscans.com
pub mod taurus;       // TaurusScan    — lectortaurus.com
pub mod leercapitulo; // LeerCapitulo  — leercapitulo.co
pub mod animeflv;     // AnimeFLV      — www3.animeflv.net
pub mod nyaa;         // Nyaa.si       — torrents de anime
pub mod kitsu;        // Kitsu.io      — metadatos de anime
pub mod unified;      // Búsqueda cross-fuente en paralelo

pub fn urlencode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        c   => format!("%{:02X}", c as u32),
    }).collect()
}

// Cliente HTTP compartido con pool de conexiones
use reqwest::blocking::Client;
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn shared_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::limited(5))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .connection_verbose(false)
            .build()
            .unwrap()
    })
}

pub fn get_html(url: &str, referer: &str) -> Result<String, String> {
    let resp = shared_client()
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
        .header("Referer", referer)
        .header("Connection", "keep-alive")
        .send()
        .map_err(|e| format!("Error de red: {}", e))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {} en {}", status, url));
    }
    if text.len() < 500 {
        return Err(format!("Respuesta corta ({} bytes)", text.len()));
    }
    Ok(text)
}
