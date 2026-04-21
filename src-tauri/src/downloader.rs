// Módulo de descarga — guarda capítulos como .pdf
// Usa spawn_blocking para no congelar la UI durante la descarga.

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use printpdf::{Image, ImageTransform, Mm, PdfDocument};

#[derive(Serialize, Deserialize)]
pub struct DownloadResult {
    pub path: String,
    pub pages: u32,
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn manga_dir(manga_title: &str, base_path: Option<&str>) -> PathBuf {
    let base = match base_path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join("Downloads").join("Foundry")
        }
    };
    base.join(sanitize(manga_title))
}

fn px_to_mm(px: u32) -> Mm {
    Mm(px as f32 / 96.0 * 25.4)
}

// Detecta la fuente y referer a partir del manga_id
fn source_info(manga_id: &str) -> (&'static str, &'static str) {
    if manga_id.starts_with("cerberus-") {
        ("cerberus", "https://legionscans.com/")
    } else if manga_id.starts_with("taurus-") {
        ("taurus", "https://lectortaurus.com/")
    } else if manga_id.starts_with("leer-") {
        ("leercapitulo", "https://leercapitulo.co/")
    } else {
        ("olympus", "https://olympusbiblioteca.com/")
    }
}

// Lógica real de descarga — se ejecuta en hilo bloqueante
fn do_download(
    manga_id: String,
    manga_title: String,
    chapter_id: String,
    chapter_title: String,
    download_path: Option<String>,
) -> Result<DownloadResult, String> {
    let (source, base_referer) = source_info(&manga_id);

    // Para fuentes no-Olympus el chapter_id ES la URL del capítulo,
    // que es el Referer correcto para las imágenes embebidas en esa página.
    // Para Olympus usamos el dominio base (el chapter_id es un ID numérico).
    let image_referer: String = match source {
        "olympus" => base_referer.to_string(),
        _         => chapter_id.clone(),
    };

    let pages = match source {
        "cerberus"     => crate::scraper::cerberus::cerberus_pages(chapter_id),
        "taurus"       => crate::scraper::taurus::taurus_pages(chapter_id, None),
        "leercapitulo" => crate::scraper::leercapitulo::leercapitulo_pages(chapter_id),
        _              => crate::scraper::olympus::get_pages(chapter_id, Some(manga_id)),
    }?;

    if pages.is_empty() {
        return Err("No se encontraron páginas en este capítulo".into());
    }

    let dir = manga_dir(&manga_title, download_path.as_deref());
    fs::create_dir_all(&dir)
        .map_err(|e| format!("No se pudo crear la carpeta: {}", e))?;

    let pdf_path = dir.join(format!("{}.pdf", sanitize(&chapter_title)));

    if pdf_path.exists() && pdf_path.metadata().map(|m| m.len()).unwrap_or(0) > 100 {
        return Ok(DownloadResult {
            path: pdf_path.to_string_lossy().to_string(),
            pages: pages.len() as u32,
        });
    }

    let client = crate::scraper::shared_client();

    // Descarga y decodifica todas las páginas
    let mut imgs: Vec<(image::DynamicImage, u32, u32)> = Vec::with_capacity(pages.len());
    for (i, page) in pages.iter().enumerate() {
        let resp = client
            .get(&page.url)
            .header("Referer", &image_referer)
            .header("Accept", "image/webp,image/avif,image/*,*/*;q=0.8")
            .send()
            .map_err(|e| format!("Error red imagen {}: {}", i + 1, e))?;

        let status = resp.status();
        let bytes = resp.bytes()
            .map_err(|e| format!("Error leyendo imagen {}: {}", i + 1, e))?;

        if !status.is_success() {
            return Err(format!(
                "[{}] HTTP {} al descargar imagen {} — {}",
                source, status, i + 1, &page.url
            ));
        }

        let dyn_img = image::load_from_memory(&bytes)
            .map_err(|e| format!("[{}] Error decodificando imagen {}: {}", source, i + 1, e))?;
        let (w, h) = (dyn_img.width(), dyn_img.height());
        imgs.push((dyn_img, w, h));
    }

    // Genera el PDF
    let (_, w0, h0) = &imgs[0];
    let (doc, first_page, first_layer) =
        PdfDocument::new(&chapter_title, px_to_mm(*w0), px_to_mm(*h0), "img");

    for (i, (dyn_img, w, h)) in imgs.iter().enumerate() {
        let (page_idx, layer_idx) = if i == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(px_to_mm(*w), px_to_mm(*h), "img")
        };
        let layer = doc.get_page(page_idx).get_layer(layer_idx);
        let pdf_img = Image::from_dynamic_image(dyn_img);
        pdf_img.add_to_layer(layer, ImageTransform {
            dpi: Some(96.0),
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            ..Default::default()
        });
    }

    let pdf_bytes = doc.save_to_bytes()
        .map_err(|e| format!("Error generando PDF: {}", e))?;

    fs::write(&pdf_path, &pdf_bytes)
        .map_err(|e| format!("Error guardando PDF: {}", e))?;

    Ok(DownloadResult {
        path: pdf_path.to_string_lossy().to_string(),
        pages: imgs.len() as u32,
    })
}

// Comando Tauri async — delega en spawn_blocking para no bloquear la UI
#[tauri::command]
pub async fn download_chapter(
    manga_id: String,
    manga_title: String,
    chapter_id: String,
    chapter_title: String,
    download_path: Option<String>,
) -> Result<DownloadResult, String> {
    tokio::task::spawn_blocking(move || {
        do_download(manga_id, manga_title, chapter_id, chapter_title, download_path)
    })
    .await
    .map_err(|e| format!("Error interno: {}", e))?
}
