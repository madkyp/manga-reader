use serde::Serialize;
use super::olympus::{self, MangaItem};
use super::{cerberus, taurus, leercapitulo};

#[derive(Serialize)]
pub struct SourceResult {
    pub source: String,
    pub label:  String,
    pub items:  Vec<MangaItem>,
}

#[tauri::command]
pub async fn unified_search(query: String) -> Vec<SourceResult> {
    if query.trim().is_empty() {
        return vec![];
    }

    let q1 = query.clone();
    let q2 = query.clone();
    let q3 = query.clone();

    // cerberus/taurus/leercapitulo son funciones bloqueantes — las ejecutamos
    // en el thread pool de Tokio para no bloquear el runtime.
    // olympus_search ya es async (hace su propio spawn_blocking internamente).
    let t_cer  = tokio::task::spawn_blocking(move || cerberus::cerberus_search(q1));
    let t_tau  = tokio::task::spawn_blocking(move || taurus::taurus_search(q2));
    let t_leer = tokio::task::spawn_blocking(move || leercapitulo::leercapitulo_search(q3));

    let (oly_res, cer_res, tau_res, leer_res) =
        tokio::join!(olympus::olympus_search(query), t_cer, t_tau, t_leer);

    let results = vec![
        SourceResult {
            source: "olympus".into(),
            label:  "Olympus".into(),
            items:  oly_res.map(|r| r.results).unwrap_or_default(),
        },
        SourceResult {
            source: "cerberus".into(),
            label:  "CerberusScan".into(),
            items:  cer_res.ok().and_then(|r| r.ok()).map(|r| r.results).unwrap_or_default(),
        },
        SourceResult {
            source: "taurus".into(),
            label:  "TaurusScan".into(),
            items:  tau_res.ok().and_then(|r| r.ok()).map(|r| r.results).unwrap_or_default(),
        },
        SourceResult {
            source: "leercapitulo".into(),
            label:  "LeerCapítulo".into(),
            items:  leer_res.ok().and_then(|r| r.ok()).map(|r| r.results).unwrap_or_default(),
        },
    ];

    // Devuelve solo las fuentes con resultados
    results.into_iter().filter(|r| !r.items.is_empty()).collect()
}
