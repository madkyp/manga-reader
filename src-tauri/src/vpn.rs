use std::process::Command;

fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr)
        })
}

fn is_installed(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn parse_connected(out: &str) -> bool {
    let lower = out.to_lowercase();
    lower.contains("connected")
        && !lower.contains("not connected")
        && !lower.contains("disconnected")
}

fn proton_status() -> String {
    if !is_installed("protonvpn-cli") {
        return "not_installed".to_string();
    }
    match run_cmd("protonvpn-cli", &["status"]) {
        Some(out) => {
            if parse_connected(&out) { "connected".to_string() } else { "disconnected".to_string() }
        }
        None => "error".to_string(),
    }
}

fn windscribe_status() -> String {
    if !is_installed("windscribe") {
        return "not_installed".to_string();
    }
    match run_cmd("windscribe", &["status"]) {
        Some(out) => {
            if parse_connected(&out) { "connected".to_string() } else { "disconnected".to_string() }
        }
        None => "error".to_string(),
    }
}

#[tauri::command]
pub async fn vpn_status(provider: String) -> String {
    tokio::task::spawn_blocking(move || match provider.as_str() {
        "proton"     => proton_status(),
        "windscribe" => windscribe_status(),
        _            => "not_installed".to_string(),
    })
    .await
    .unwrap_or_else(|_| "error".to_string())
}

#[tauri::command]
pub async fn vpn_connect(provider: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || match provider.as_str() {
        "proton" => {
            if !is_installed("protonvpn-cli") {
                return Err("ProtonVPN CLI no instalado".to_string());
            }
            match run_cmd("protonvpn-cli", &["connect", "--fastest"]) {
                Some(out) => {
                    if parse_connected(&out) || !out.to_lowercase().contains("error") {
                        Ok("connected".to_string())
                    } else {
                        Err(out.trim().to_string())
                    }
                }
                None => Err("Error al ejecutar protonvpn-cli".to_string()),
            }
        }
        "windscribe" => {
            if !is_installed("windscribe") {
                return Err("Windscribe CLI no instalado".to_string());
            }
            match run_cmd("windscribe", &["connect", "best"]) {
                Some(out) => {
                    if out.to_lowercase().contains("error") || out.to_lowercase().contains("fail") {
                        Err(out.trim().to_string())
                    } else {
                        Ok("connected".to_string())
                    }
                }
                None => Err("Error al ejecutar windscribe".to_string()),
            }
        }
        _ => Err("Proveedor no soportado".to_string()),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn vpn_disconnect(provider: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || match provider.as_str() {
        "proton" => {
            if !is_installed("protonvpn-cli") {
                return Err("ProtonVPN CLI no instalado".to_string());
            }
            match run_cmd("protonvpn-cli", &["disconnect"]) {
                Some(_) => Ok("disconnected".to_string()),
                None    => Err("Error al desconectar ProtonVPN".to_string()),
            }
        }
        "windscribe" => {
            if !is_installed("windscribe") {
                return Err("Windscribe CLI no instalado".to_string());
            }
            match run_cmd("windscribe", &["disconnect"]) {
                Some(_) => Ok("disconnected".to_string()),
                None    => Err("Error al desconectar Windscribe".to_string()),
            }
        }
        _ => Err("Proveedor no soportado".to_string()),
    })
    .await
    .map_err(|e| e.to_string())?
}
