use serde_json::json;
use base64::Engine;
use sqlx::Row;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

fn chapter_number_from_name(name: &str) -> u32 {
    let mut digits = String::new();
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    digits.parse::<u32>().unwrap_or(0)
}

fn is_image_path(p: &str) -> bool {
    let low = p.to_ascii_lowercase();
    low.ends_with(".jpg") || low.ends_with(".jpeg") || low.ends_with(".png") || low.ends_with(".webp")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn mime_from_path(name: &str) -> &'static str {
    let low = name.to_ascii_lowercase();
    if low.ends_with(".png") {
        "image/png"
    } else if low.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

fn bytes_to_data_url(bytes: &[u8], mime: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{};base64,{}", mime, encoded)
}

async fn get_manga_local_path(manga_id: &str) -> Result<PathBuf, String> {
    let pool = crate::config::database::get_connection_pool()
        .await
        .map_err(|e| format!("DB pool error: {}", e))?;

    let row_opt = sqlx::query("SELECT local_path FROM manga WHERE id = ?")
        .bind(manga_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("DB query error: {}", e))?;

    let row = row_opt.ok_or_else(|| format!("Manga not found: {}", manga_id))?;
    let local_path: String = row
        .try_get("local_path")
        .map_err(|e| format!("Invalid local_path: {}", e))?;

    if local_path.trim().is_empty() {
        return Err("Manga local_path is empty".to_string());
    }

    Ok(PathBuf::from(local_path))
}

#[tauri::command]
pub async fn list_local_chapters(manga_id: String) -> Result<serde_json::Value, String> {
    let local_root = get_manga_local_path(&manga_id).await?;
    if !local_root.exists() {
        return Err(format!("Manga path does not exist: {}", local_root.display()));
    }

    let mut items: Vec<(u32, String, String)> = Vec::new();

    // Prefer CBZ files (single source of truth for chapter reading)
    if let Ok(entries) = fs::read_dir(&local_root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                let is_cbz = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("cbz"))
                    .unwrap_or(false);
                if is_cbz {
                    let name = p
                        .file_name()
                        .and_then(|x| x.to_str())
                        .unwrap_or("chapter.cbz")
                        .to_string();
                    let n = chapter_number_from_name(&name);
                    items.push((n, format!("cbz::{}", name), format!("Cap {}", if n > 0 { n } else { 0 })));
                }
            }
        }
    }

    // Fallback to chapter directories only when there are no cbz files
    if items.is_empty() {
        if let Ok(entries) = fs::read_dir(&local_root) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = p
                        .file_name()
                        .and_then(|x| x.to_str())
                        .unwrap_or("chapter")
                        .to_string();
                    let n = chapter_number_from_name(&name);
                    items.push((n, format!("dir::{}", name), format!("Cap {}", if n > 0 { n } else { 0 })));
                }
            }
        }
    }

    items.sort_by(|a, b| {
        if a.0 == b.0 {
            a.1.cmp(&b.1)
        } else {
            a.0.cmp(&b.0)
        }
    });

    let chapters: Vec<serde_json::Value> = items
        .into_iter()
        .map(|(n, id, title)| {
            json!({
                "id": id,
                "number": n,
                "title": title
            })
        })
        .collect();

    Ok(json!({ "chapters": chapters }))
}

#[tauri::command]
pub async fn get_chapter_pages(manga_id: String, chapter_id: String) -> Result<serde_json::Value, String> {
    log::info!("Getting pages for manga {} chapter {}", manga_id, chapter_id);
    let local_root = get_manga_local_path(&manga_id).await?;

    let mut pages: Vec<String> = Vec::new();

    if let Some(file_name) = chapter_id.strip_prefix("cbz::") {
        let cbz_path = local_root.join(file_name);
        if !cbz_path.exists() {
            return Err(format!("CBZ not found: {}", cbz_path.display()));
        }

        let f = fs::File::open(&cbz_path).map_err(|e| format!("Failed to open CBZ: {}", e))?;
        let mut archive = zip::ZipArchive::new(f).map_err(|e| format!("Invalid CBZ archive: {}", e))?;

        let mut image_entries: Vec<String> = Vec::new();
        for i in 0..archive.len() {
            if let Ok(zf) = archive.by_index(i) {
                let name = zf.name().to_string();
                if !zf.is_dir() && is_image_path(&name) {
                    image_entries.push(name);
                }
            }
        }
        image_entries.sort();

        for entry_name in image_entries {
            let mut zf = archive
                .by_name(&entry_name)
                .map_err(|e| format!("Failed to read entry {}: {}", entry_name, e))?;
            let mut buf = Vec::new();
            zf.read_to_end(&mut buf)
                .map_err(|e| format!("Failed to extract image bytes: {}", e))?;
            let mime = mime_from_path(&entry_name);
            pages.push(bytes_to_data_url(&buf, mime));
        }
    } else {
        // Directory fallback
        let dir_name = chapter_id
            .strip_prefix("dir::")
            .unwrap_or(chapter_id.as_str());
        let chapter_dir = local_root.join(dir_name);
        if !chapter_dir.exists() {
            return Err(format!("Chapter folder not found: {}", chapter_dir.display()));
        }

        let mut file_paths: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(&chapter_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_file() {
                    let low = p.to_string_lossy().to_ascii_lowercase();
                    if is_image_path(&low) {
                        file_paths.push(p);
                    }
                }
            }
        }

        file_paths.sort();
        for p in file_paths {
            let bytes = fs::read(&p).map_err(|e| format!("Failed to read page file {}: {}", p.display(), e))?;
            let mime = mime_from_path(&path_to_string(&p));
            pages.push(bytes_to_data_url(&bytes, mime));
        }
    }

    Ok(json!({
        "pages": pages,
        "total_pages": pages.len()
    }))
}

#[tauri::command]
pub async fn mark_chapter_read(_manga_id: String, _chapter_id: String, current_page: u32, total_pages: u32) -> Result<serde_json::Value, String> {
    log::info!("Marking chapter as read: page {}/{}", current_page, total_pages);

    Ok(json!({
        "status": "success"
    }))
}
