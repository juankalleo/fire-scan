use serde_json::json;
use base64::Engine;
use sqlx::SqlitePool;
use std::path::PathBuf;
use crate::config::init as config_init;
use tauri::Manager;
use std::fs;
use crate::library::library_service::Manga as MangaStruct;
use std::path::Path;
use walkdir::WalkDir;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;
use crate::library::sqlite_repository::MangaRepository;

fn cover_file_to_data_url(path: &str) -> Option<String> {
    let p = Path::new(path);
    if !p.exists() || !p.is_file() {
        return None;
    }

    let mime = match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => return None,
    };

    let bytes = std::fs::read(p).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{};base64,{}", mime, b64))
}

// State para manter pool de conexão e caminhos
#[allow(dead_code)]
pub struct AppState {
    pub db_pool: Option<SqlitePool>,
    pub library_path: PathBuf,
}

#[tauri::command]
pub async fn get_library(
    page: u32,
    page_size: u32,
    state: tauri::State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Fetching library page {} with page size {}", page, page_size);

    let pool_opt = {
        let state = state.lock().await;
        state.db_pool.clone()
    };

    // If DB pool not ready yet, return an empty page rather than panicking
    if pool_opt.is_none() {
        return Ok(serde_json::json!({"page": page, "pageSize": page_size, "totalPages": 0, "items": []}));
    }

    let pool = pool_opt.as_ref().unwrap();

    // Query database
    match crate::library::MangaRepository::list_manga(pool, page, page_size).await {
        Ok((manga_list, total_pages)) => {
            let mut items = Vec::new();

            for mut m in manga_list {
                if looks_like_chapter_entry(&m.title, &m.local_path) {
                    // Clean up legacy noisy records imported as chapter_1/chapter_2.
                    if let Err(e) = MangaRepository::delete_manga(pool, &m.id).await {
                        log::warn!("Failed to delete polluted chapter entry {}: {}", m.id, e);
                    }
                    continue;
                }

                // Keep chapter counts in sync with what is actually present on disk.
                // This also fixes legacy rows that double-counted chapter folders + .cbz files.
                let repaired = count_chapters_for_local_path(&m.local_path);
                if repaired > 0 && (m.total_chapters != repaired || m.downloaded_chapters != repaired) {
                    m.total_chapters = repaired;
                    m.downloaded_chapters = repaired;
                    if let Err(e) = MangaRepository::upsert_manga(pool, &m).await {
                        log::warn!("Failed to persist repaired chapter counts for {}: {}", m.id, e);
                    }
                }

                let cover_for_ui = m.cover_path.as_ref().and_then(|cp| {
                    let low = cp.to_lowercase();
                    if low.starts_with("http://")
                        || low.starts_with("https://")
                        || low.starts_with("data:")
                        || low.starts_with("asset://")
                        || low.starts_with("tauri://")
                    {
                        Some(cp.clone())
                    } else {
                        cover_file_to_data_url(cp).or_else(|| Some(cp.clone()))
                    }
                });

                items.push(json!({
                    "id": m.id,
                    "title": m.title,
                    "sourceName": m.source_name,
                    "coverPath": cover_for_ui,
                    "synopsis": m.synopsis,
                    "status": m.status,
                    "rating": m.rating,
                    "language": m.language,
                    "localPath": m.local_path,
                    "totalChapters": m.total_chapters,
                    "downloadedChapters": m.downloaded_chapters,
                    "lastUpdated": m.last_updated,
                }));
            }

            Ok(json!({
                "page": page,
                "pageSize": page_size,
                "totalPages": total_pages,
                "items": items
            }))
        }
        Err(e) => {
            log::error!("Failed to fetch library: {}", e);
            Err(format!("Failed to fetch library: {}", e))
        }
    }
}

fn looks_like_chapter_entry(title: &str, local_path: &str) -> bool {
    let t = title.trim().to_lowercase();
    if t.starts_with("chapter_") || t.starts_with("chapter-") || t.starts_with("chapter ")
        || t.starts_with("capitulo_") || t.starts_with("capitulo-") || t.starts_with("capitulo ") {
        return true;
    }

    let leaf = Path::new(local_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    leaf.starts_with("chapter_")
        || leaf.starts_with("chapter-")
        || leaf.starts_with("chapter ")
        || leaf.starts_with("capitulo_")
        || leaf.starts_with("capitulo-")
        || leaf.starts_with("capitulo ")
}

fn count_chapters_for_local_path(local_path: &str) -> i32 {
    let p = Path::new(local_path);
    if !p.exists() {
        return 0;
    }

    if p.is_file() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        return if ext == "cbz" || ext == "zip" { 1 } else { 0 };
    }

    let mut count = 0i32;

    // Root files and chapter_* folders.
    if let Ok(entries) = fs::read_dir(p) {
        for e in entries.flatten() {
            let path = e.path();
            if let Ok(ft) = e.file_type() {
                if ft.is_file() {
                    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
                    if ext == "cbz" || ext == "zip" {
                        count += 1;
                    }
                } else if ft.is_dir() {
                    let n = path.file_name().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
                    if n == "chapters" {
                        if let Ok(ch_entries) = fs::read_dir(path) {
                            count += ch_entries
                                .flatten()
                                .filter(|c| c.file_type().map(|t| t.is_file()).unwrap_or(false))
                                .filter(|c| {
                                    let ext = c.path().extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
                                    ext == "cbz" || ext == "zip"
                                })
                                .count() as i32;
                        }
                    }
                }
            }
        }
    }

    // If index.json exists with chapter metadata, prefer that when larger.
    let index_path = p.join("index.json");
    if index_path.exists() {
        if let Ok(txt) = fs::read_to_string(index_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(obj) = v.get("chapters").and_then(|x| x.as_object()) {
                    let indexed = obj.len() as i32;
                    if indexed > count {
                        count = indexed;
                    }
                }
            }
        }
    }

    count
}

#[tauri::command]
pub async fn get_library_path_cmd(
    state: tauri::State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    // Read the authoritative library path from config (this respects the
    // persisted override file) so the frontend always gets the latest value
    // even if the managed AppState hasn't been updated yet.
    match config_init::get_library_path() {
        Ok(p) => Ok(json!({"library_path": p.to_string_lossy().to_string()})),
        Err(e) => {
            log::error!("Failed to read library path from config: {}", e);
            // Fall back to the managed state to avoid a hard failure
            let s = state.lock().await;
            Ok(json!({"library_path": s.library_path.to_string_lossy().to_string()}))
        }
    }
}

#[tauri::command]
pub async fn set_library_path_cmd(
    new_path: String,
    state: tauri::State<'_, tokio::sync::Mutex<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let trimmed = new_path.trim();
    if trimmed.is_empty() {
        return Err("Library path cannot be empty".to_string());
    }

    let pb = PathBuf::from(trimmed);

    if let Err(e) = std::fs::create_dir_all(&pb) {
        log::error!("Failed to create library directory {:?}: {}", pb, e);
        return Err(format!("Failed to create library directory: {}", e));
    }

    // persist
    if let Err(e) = config_init::set_library_path(&pb) {
        log::error!("Failed to persist library path override: {}", e);
        return Err(format!("Failed to persist library path: {}", e));
    }

    // read current pool and update managed state
    let pool_opt = {
        let mut s = state.lock().await;
        s.library_path = pb.clone();
        s.db_pool.clone()
    };

    // Apply switch immediately by rebuilding local library index from the new path.
    if let Some(pool) = pool_opt {
        if let Err(e) = sqlx::query("DELETE FROM manga WHERE source_id = 'local'")
            .execute(&pool)
            .await
        {
            log::warn!("Failed to clear local manga entries after library path change: {}", e);
        }

        match crate::library::LibraryService::scan_library(&pb).await {
            Ok(manga_list) => {
                for manga in manga_list {
                    if let Err(e) = crate::library::MangaRepository::upsert_manga(&pool, &manga).await {
                        log::warn!("Failed to upsert manga during library path switch: {}", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Library rescan after path switch failed: {}", e);
            }
        }

        if let Ok((list, _)) = crate::library::sqlite_repository::MangaRepository::list_manga(&pool, 1, 1000).await {
            if let Ok(items_json) = serde_json::to_value(list) {
                let _ = app_handle.emit_all("library-updated", serde_json::json!({"status":"ok","items": items_json}));
            }
        }
    }

    Ok(json!({"library_path": pb.to_string_lossy().to_string()}))
}

#[tauri::command]
pub async fn force_rescan_and_refresh(
    state: tauri::State<'_, tokio::sync::Mutex<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    log::info!("Force rescan requested");

    // Get authoritative library path
    let lib_path = match config_init::get_library_path() {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to read library path: {}", e);
            return Err(format!("Failed to read library path: {}", e));
        }
    };

    // Get DB pool from managed state
    let pool_opt = {
        let s = state.lock().await;
        s.db_pool.clone()
    };

    let pool = match pool_opt {
        Some(p) => p,
        None => return Err("Database not ready".to_string()),
    };

    // Scan top-level directories in the library path and upsert
    let mut count = 0usize;
    match fs::read_dir(&lib_path) {
        Ok(entries) => {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if title.is_empty() { continue; }

                let title_l = title.to_lowercase();
                // Never index chapter subfolders as independent manga.
                if title_l.starts_with("chapter_")
                    || title_l.starts_with("chapter-")
                    || title_l.starts_with("chapter ")
                    || title_l.starts_with("capitulo_")
                    || title_l.starts_with("capitulo-")
                    || title_l.starts_with("capitulo ")
                {
                    continue;
                }

                let id = format!("local_{}", title.replace(' ', "_").to_lowercase());

                // find cover
                let mut cover: Option<String> = None;
                for candidate in &["cover.jpg", "cover.png", "cover.webp"] {
                    let p = path.join(candidate);
                    if p.exists() {
                        cover = Some(p.to_string_lossy().to_string());
                        break;
                    }
                }
                if cover.is_none() {
                    // pick first image file found
                    if let Ok(mut diriter) = fs::read_dir(&path) {
                        for f in diriter.filter_map(|e| e.ok()) {
                            if let Some(ext) = f.path().extension().and_then(|e| e.to_str()) {
                                let ext = ext.to_lowercase();
                                if ["jpg","jpeg","png","webp"].contains(&ext.as_str()) {
                                    cover = Some(f.path().to_string_lossy().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }

                // If still none, try to generate a cover by picking a random image
                // from the manga folder (including extracting first images from CBZ/ZIP files)
                if cover.is_none() {
                    if let Some(generated) = generate_random_cover_for_dir(&path, &id) {
                        cover = Some(generated);
                    }
                }

                let manga = MangaStruct {
                    id: id.clone(),
                    title: title.clone(),
                    source_id: String::from("local"),
                    source_name: String::from("Local"),
                    cover_path: cover.clone(),
                    synopsis: Some(format!("Imported from folder: {}", path.to_string_lossy())),
                    status: String::from("unknown"),
                    rating: 0.0,
                    language: String::from("pt-BR"),
                    local_path: path.to_string_lossy().to_string(),
                    total_chapters: 0,
                    downloaded_chapters: 0,
                    last_updated: chrono::Local::now().to_rfc3339(),
                };

                if let Err(e) = crate::library::sqlite_repository::MangaRepository::upsert_manga(&pool, &manga).await {
                    log::error!("Failed to upsert manga {}: {}", id, e);
                } else {
                    count += 1;
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read library dir {}: {}", lib_path.to_string_lossy(), e);
            return Err(format!("Failed to read library dir: {}", e));
        }
    }

    // Update managed AppState library_path (db_pool already set)
    {
        let mut s = state.lock().await;
        s.library_path = lib_path.clone();
    }

    // Emit event to frontend that library updated
    // Build and emit full library items for the UI
    let items_json = match crate::library::sqlite_repository::MangaRepository::list_manga(&pool, 1, 1000).await {
        Ok((list, _)) => serde_json::to_value(list).unwrap_or(json!([])),
        Err(e) => {
            log::error!("Failed to query manga list for event: {}", e);
            json!([])
        }
    };

    let payload = json!({"status":"ok","imported": count, "items": items_json});
    if let Err(e) = app_handle.emit_all("library-updated", payload.clone()) {
        log::warn!("Failed to emit library-updated event: {}", e);
    }

    // Also emit downloads snapshot
    let downloads = crate::commands::download::get_downloads_snapshot().await;
    if let Err(e) = app_handle.emit_all("downloads-updated", downloads.clone()) {
        log::warn!("Failed to emit downloads-updated event: {}", e);
    }

    // After import, attempt to generate missing covers for newly imported entries
    match crate::commands::library::generate_missing_covers_startup(&pool, app_handle.clone()).await {
        Ok(n) => log::info!("generate_missing_covers_startup after import updated {} covers", n),
        Err(e) => log::warn!("generate_missing_covers_startup after import failed: {}", e),
    }

    Ok(json!({"imported": count}))
}

fn generate_random_cover_for_dir(dir: &Path, id: &str) -> Option<String> {
    // Determine covers dir under app data
    let covers_dir = match config_init::get_app_dirs() {
        Ok(d) => d.data_local_dir().join("covers"),
        Err(_) => PathBuf::from("C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\covers"),
    };

    if let Err(e) = std::fs::create_dir_all(&covers_dir) {
        log::warn!("Failed to create covers dir {}: {}", covers_dir.display(), e);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    // collect image files and extract first image from CBZ/ZIP files
    let mut cbz_extract_index = 0usize;
    if dir.exists() {
        for entry in WalkDir::new(dir).max_depth(8).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() { continue; }
            if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                let ext_l = ext.to_lowercase();
                if ["jpg","jpeg","png","webp"].contains(&ext_l.as_str()) {
                    candidates.push(p.to_path_buf());
                    continue;
                }
                if ext_l == "cbz" || ext_l == "zip" {
                    // try open and extract first image entry to covers_dir as a candidate
                    if let Ok(file) = std::fs::File::open(&p) {
                        if let Ok(mut archive) = ZipArchive::new(file) {
                            for i in 0..archive.len() {
                                if let Ok(mut f) = archive.by_index(i) {
                                    if let Some(name_ext) = Path::new(f.name()).extension().and_then(|s| s.to_str()) {
                                        let name_ext_l = name_ext.to_lowercase();
                                        if ["jpg","jpeg","png","webp"].contains(&name_ext_l.as_str()) {
                                            let out_path = covers_dir.join(format!("{}_cbz_{}.{}", id, cbz_extract_index, name_ext_l));
                                            if let Ok(mut out) = std::fs::File::create(&out_path) {
                                                if std::io::copy(&mut f, &mut out).is_ok() {
                                                    candidates.push(out_path);
                                                    cbz_extract_index += 1;
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // pick a pseudo-random candidate using system time
    let idx = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() as usize) % candidates.len())
        .unwrap_or(0);

    let chosen = &candidates[idx];

    // Copy/normalize chosen image into covers_dir as {id}.{ext}
    if let Some(ext) = chosen.extension().and_then(|s| s.to_str()) {
        let target = covers_dir.join(format!("{}.{}", id, ext.to_lowercase()));
        if let Err(e) = std::fs::copy(chosen, &target) {
            log::warn!("Failed to copy chosen cover {:?} -> {:?}: {}", chosen, target, e);
            return None;
        }
        return Some(target.to_string_lossy().to_string());
    }

    None
}

/// Generate missing covers for all mangas in the DB. Returns number of mangas updated.
pub async fn generate_missing_covers_startup(pool: &SqlitePool, app_handle: tauri::AppHandle) -> Result<usize, String> {
    log::info!("generate_missing_covers_startup: scanning DB for missing covers");
    let (manga_list, _) = match MangaRepository::list_manga(pool, 1, 1000).await {
        Ok((l, p)) => (l, p),
        Err(e) => return Err(format!("Failed to list manga: {}", e)),
    };

    let mut updated = 0usize;
    for mut m in manga_list {
        let has_cover = m.cover_path.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
        if has_cover {
            continue;
        }

        let dir = Path::new(&m.local_path);
        if let Some(new_cover) = generate_random_cover_for_dir(dir, &m.id) {
            log::info!("Generated cover for {} -> {}", m.id, new_cover);
            m.cover_path = Some(new_cover.clone());
            if let Err(e) = MangaRepository::upsert_manga(pool, &m).await {
                log::warn!("Failed to update manga {} with new cover: {}", m.id, e);
            } else {
                updated += 1;
            }
        }
    }

    // Emit updated library list if we changed anything
    if updated > 0 {
        match MangaRepository::list_manga(pool, 1, 1000).await {
            Ok((list, _)) => {
                if let Ok(items_json) = serde_json::to_value(list) {
                    let _ = app_handle.emit_all("library-updated", serde_json::json!({"status":"ok","items": items_json}));
                }
            }
            Err(e) => log::warn!("Failed to query manga for post-update emit: {}", e),
        }
    }

    Ok(updated)
}
