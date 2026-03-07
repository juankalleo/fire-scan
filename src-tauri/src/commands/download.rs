use serde_json::json;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;
use uuid::Uuid;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::commands::library::AppState;
use std::time::SystemTime;
use sqlx::Row;
use crate::library::LibraryService;
use crate::library::sqlite_repository::MangaRepository;
use crate::scraper::WebScraper;
use reqwest::Url;
use tokio::fs;
use zip::write::FileOptions;
use std::io::Write;

static DOWNLOADS: Lazy<Mutex<HashMap<String, serde_json::Value>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn slug_from_url(url: &str, fallback: &str) -> String {
    // Handle URLs with trailing slash by taking last non-empty segment.
    let trimmed = url.trim_end_matches('/');
    let seg = trimmed
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(fallback);
    let slug = seg.replace(|c: char| !c.is_alphanumeric(), "_");
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

fn has_recent_download_content(path: &std::path::Path, since: SystemTime) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            if let Ok(meta) = e.metadata() {
                if let Ok(modified) = meta.modified() {
                    if modified >= since {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn find_cover_in_downloaded_tree(root: &std::path::Path) -> Option<String> {
    // 1) Known cover names at root.
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            if let Ok(ft) = e.file_type() {
                if ft.is_file() {
                    if let Some(fname) = e.file_name().to_str() {
                        let low = fname.to_lowercase();
                        if low == "cover.jpg" || low == "cover.png" || low == "cover.webp" {
                            return Some(e.path().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    // 2) First image found recursively in chapter dirs.
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    if let Ok(inner) = std::fs::read_dir(e.path()) {
                        for ie in inner.flatten() {
                            if let Ok(ift) = ie.file_type() {
                                if ift.is_file() {
                                    if let Some(iname) = ie.file_name().to_str() {
                                        let low = iname.to_lowercase();
                                        if low.ends_with(".jpg") || low.ends_with(".jpeg") || low.ends_with(".png") || low.ends_with(".webp") {
                                            return Some(ie.path().to_string_lossy().to_string());
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

    None
}

fn count_downloaded_chapters_in_tree(root: &std::path::Path) -> i32 {
    let mut chapter_count = 0i32;

    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let path = e.path();

            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.eq_ignore_ascii_case("chapters") {
                            if let Ok(ch_entries) = std::fs::read_dir(&path) {
                                chapter_count += ch_entries
                                    .flatten()
                                    .filter(|c| c.file_type().map(|t| t.is_file()).unwrap_or(false))
                                    .filter(|c| {
                                        c.file_name()
                                            .to_str()
                                            .map(|n| n.to_lowercase().ends_with(".cbz"))
                                            .unwrap_or(false)
                                    })
                                    .count() as i32;
                            }
                        }
                    }
                }

                if ft.is_file() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.to_lowercase().ends_with(".cbz") {
                            chapter_count += 1;
                        }
                    }
                }
            }
        }
    }

    chapter_count
}

async fn try_fetch_cover_by_title_to_local(
    title_hint: &str,
    per_manga_dest: &std::path::Path,
) -> Option<String> {
    if title_hint.trim().is_empty() {
        return None;
    }

    // Use existing web search integration and choose first result with a usable cover URL.
    let candidates = WebScraper::search_niadd(title_hint).await.ok()?;
    let cover_url = candidates
        .into_iter()
        .find_map(|m| m.cover_path)
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))?;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;

    let resp = client.get(&cover_url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() {
        return None;
    }

    let low = cover_url.to_lowercase();
    let ext = if low.contains(".webp") {
        "webp"
    } else if low.contains(".png") {
        "png"
    } else {
        "jpg"
    };
    let out = per_manga_dest.join(format!("cover_web.{}", ext));
    if fs::write(&out, &bytes).await.is_ok() {
        Some(out.to_string_lossy().to_string())
    } else {
        None
    }
}

// Return a snapshot of current downloads for UI consumption
pub async fn get_downloads_snapshot() -> serde_json::Value {
    let map = DOWNLOADS.lock().await;
    let mut arr = Vec::new();
    for (id, v) in map.iter() {
        let mut obj = v.clone();
        if let Some(m) = obj.as_object_mut() {
            m.insert("id".to_string(), serde_json::Value::String(id.clone()));
        }
        arr.push(obj.clone());
    }
    serde_json::Value::Array(arr)
}

fn find_kotatsu_jar() -> Option<PathBuf> {
    // 0) Highest priority: explicit override via environment variable
    if let Ok(p) = std::env::var("KOTATSU_DL_JAR") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }

    // Search for kotatsu-dl.jar in a few likely locations relative to
    // - current working dir
    // - the running executable directory
    // - up to a few parent directories
    let candidates = ["kotatsu-dl.jar", "kotatsu-dl-0.8/kotatsu-dl.jar"];

    // 1) Check current working directory and a few parents
    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..5 {
            for name in &candidates {
                let p = dir.join(name);
                if p.exists() {
                    return Some(p);
                }
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    // 2) Check the directory containing the running executable
    if let Ok(mut exe_dir) = std::env::current_exe() {
        if let Some(d) = exe_dir.parent() {
            let mut dir = d.to_path_buf();
            for _ in 0..5 {
                for name in &candidates {
                    let p = dir.join(name);
                    if p.exists() {
                        return Some(p);
                    }
                }
                if let Some(parent) = dir.parent() {
                    dir = parent.to_path_buf();
                } else {
                    break;
                }
            }
        }
    }

    // 3) Fallback: try a few simple relative names (legacy behaviour)
    for name in &candidates {
        let p = PathBuf::from(name);
        if p.exists() {
            return Some(p);
        }
    }

    // 4) App data fallback (works for installed app runs outside repo)
    if let Ok(dirs) = crate::config::init::get_app_dirs() {
        let data_dir = dirs.data_local_dir();
        let app_data_candidates = [
            data_dir.join("kotatsu-dl.jar"),
            data_dir.join("tools").join("kotatsu-dl.jar"),
            data_dir.join("bin").join("kotatsu-dl.jar"),
        ];
        for p in &app_data_candidates {
            if p.exists() {
                return Some(p.clone());
            }
        }
    }

    // 5) Absolute workspace-like fallback on Windows dev environments
    if cfg!(target_os = "windows") {
        let known = [
            r"C:\Users\combo\Documents\projetos\FireScan\kotatsu-dl.jar",
            r"C:\Users\combo\Documents\projetos\FireScan\kotatsu-dl-0.8\kotatsu-dl.jar",
            r"C:\Users\combo\Documents\projetos\FireScan\kotatsu-dl-0.8\build\libs\kotatsu-dl.jar",
        ];
        for p in known.iter().map(PathBuf::from) {
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

#[tauri::command]
pub async fn start_download(
    url: String,
    chapters: String,
    format: String,
    state: tauri::State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Manual download requested: url='{}' chapters='{}' format='{}'", url, chapters, format);

    let jar = match find_kotatsu_jar() {
        Some(p) => p,
        None => return Err("kotatsu-dl.jar not found. Set KOTATSU_DL_JAR or place it in repo root, kotatsu-dl-0.8/, or app data tools folder.".to_string()),
    };
    log::info!("Using kotatsu jar at: {}", jar.display());

    let download_id = Uuid::new_v4().to_string();

    // capture DB pool and library path from state
    let app_state = state.lock().await;
    let pool = app_state.db_pool.clone();
    let library_path = app_state.library_path.clone();

    // Prefer persisted library override (library_path.txt) when available
    let dest = match crate::config::init::get_library_path() {
        Ok(p) => p,
        Err(_) => library_path.clone(),
    };

    // Prepare clones for background task
    let dest_clone = dest.clone();
    let download_id_clone = download_id.clone();
    let pool_clone = pool.clone();
    let library_path_clone = library_path.clone();

    // capture start time to find newly created files
    let start_time = SystemTime::now();

    // Insert initial status and mark as running immediately so callers receive active status
    {
        let mut map = DOWNLOADS.lock().await;
        map.insert(download_id.clone(), json!({"status":"running","dest": dest.to_string_lossy().to_string(), "progress": 0}));
    }

    // Spawn background task to run kotatsu jar and stream output for progress updates
    tokio::spawn(async move {
        log::info!("Starting kotatsu subprocess: {:?}", jar);

        // Build a per-manga dest folder so kotatsu writes into a dedicated folder
        let title_slug = slug_from_url(&url, &download_id_clone);
        let per_manga_dest = dest_clone.join(&title_slug);
        if let Err(e) = std::fs::create_dir_all(&per_manga_dest) {
            log::warn!("Failed to create per-manga dest {:?}: {}", per_manga_dest, e);
        }
        log::info!("Manual download destination resolved to: {}", per_manga_dest.display());

        // Before invoking kotatsu, check whether the jar supports the source
        // for this URL (quick --sources probe). If not, record an error
        // so the UI doesn't remain stuck in 'pending'.
        let jar_dir = jar.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let jar_str = jar.to_string_lossy().to_string();
        let mut check_cmd = Command::new("java");
        check_cmd.current_dir(&jar_dir);
        check_cmd.args(&["-jar", jar_str.as_str(), "--sources"]);
        if let Ok(output) = check_cmd.output().await {
            let out = String::from_utf8_lossy(&output.stdout).to_lowercase();
            let url_l = url.to_lowercase();
                    if url_l.contains("niadd") && !out.contains("niadd") {
                        log::warn!("kotatsu jar does not include Niadd parser; falling back to internal downloader for Niadd");
                        // mark as running so UI/pollers see active state
                        {
                            let mut map = DOWNLOADS.lock().await;
                            map.insert(download_id_clone.clone(), json!({"status":"running","dest": per_manga_dest.to_string_lossy().to_string(), "progress": 0}));
                        }
                        // call internal downloader implementation
                        if let Err(e) = internal_download_niadd(&url, &per_manga_dest, &chapters, &format, &download_id_clone, &pool_clone, start_time).await {
                            log::error!("internal niadd download failed: {}", e);
                            let mut map = DOWNLOADS.lock().await;
                            map.insert(download_id_clone.clone(), json!({"status":"error","error": e.to_string()}));
                        }
                        return;
                    }
                    if url_l.contains("mangalivre") && !out.contains("mangalivre") {
                        log::warn!("kotatsu jar does not include MangaLivre parser; falling back to internal downloader for MangaLivre");
                        {
                            let mut map = DOWNLOADS.lock().await;
                            map.insert(download_id_clone.clone(), json!({"status":"running","dest": per_manga_dest.to_string_lossy().to_string(), "progress": 0}));
                        }
                        if let Err(e) = internal_download_mangalivre(&url, &per_manga_dest, &chapters, &format, &download_id_clone, &pool_clone, start_time).await {
                            log::error!("internal mangalivre download failed: {}", e);
                            let mut map = DOWNLOADS.lock().await;
                            map.insert(download_id_clone.clone(), json!({"status":"error","error": e.to_string()}));
                        }
                        return;
                    }
        }

        // kotatsu expects options before the <link> argument; place URL last
        let args = vec![
            "-jar".to_string(),
            jar.to_string_lossy().to_string(),
            "--dest".to_string(),
            per_manga_dest.to_string_lossy().to_string(),
            "--format".to_string(),
            format.clone(),
            "--chapters".to_string(),
            chapters.clone(),
            url.clone(),
        ];

        let mut cmd = Command::new("java");
        // Ensure the process runs with the jar's parent directory as CWD so
        // kotatsu can locate bundled parser resources if needed.
        if let Some(parent_dir) = jar.parent() {
            cmd.current_dir(parent_dir);
        }
        cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                // Update initial entry with per-manga dest and pid
                {
                    let mut map = DOWNLOADS.lock().await;
                    map.insert(download_id_clone.clone(), json!({"status":"running","pid":child.id().unwrap_or(0),"dest": per_manga_dest.to_string_lossy().to_string(),"progress":0}));
                }

                // Stream stdout
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    let download_id_stdout = download_id_clone.clone();
                    tokio::spawn(async move {
                        while let Ok(Some(line)) = lines.next_line().await {
                            log::info!("kotatsu: {}", line);
                            // Try to parse percentage like "12%" or "[12%]"
                            let mut progress: Option<u8> = None;
                            if let Some(p_pos) = line.find('%') {
                                // get number before %
                                let snippet = &line[..p_pos];
                                if let Some(num_str) = snippet.split_whitespace().rev().next() {
                                    if let Ok(v) = num_str.trim().trim_matches(|c: char| !c.is_numeric()).parse::<u8>() {
                                        progress = Some(v.min(100));
                                    }
                                }
                            }

                            // Update map with last stdout and progress if found
                            let mut map = futures::executor::block_on(async { DOWNLOADS.lock().await });
                            if let Some(mut entry) = map.get_mut(&download_id_stdout) {
                                let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                                obj.insert("last_stdout".to_string(), serde_json::Value::String(line.clone()));
                                if let Some(p) = progress {
                                    obj.insert("progress".to_string(), serde_json::Value::Number(serde_json::Number::from(p)));
                                }
                                *entry = serde_json::Value::Object(obj);
                            }
                        }
                    });
                }

                // Stream stderr similarly
                let parser_dummy_detected = Arc::new(AtomicBool::new(false));
                if let Some(stderr) = child.stderr.take() {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    let download_id_stderr = download_id_clone.clone();
                    let parser_dummy_detected_stderr = parser_dummy_detected.clone();
                    tokio::spawn(async move {
                        while let Ok(Some(line)) = lines.next_line().await {
                            log::warn!("kotatsu stderr: {}", line);
                            let low = line.to_lowercase();
                            if low.contains("parser dummy") || low.contains("cannot be instantiated") {
                                parser_dummy_detected_stderr.store(true, Ordering::Relaxed);
                            }
                            let mut map = futures::executor::block_on(async { DOWNLOADS.lock().await });
                            if let Some(mut entry) = map.get_mut(&download_id_stderr) {
                                let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                                obj.insert("last_stderr".to_string(), serde_json::Value::String(line.clone()));
                                *entry = serde_json::Value::Object(obj);
                            }
                        }
                    });
                }

                // Wait for process to finish
                let wait_status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Ok(Some(status)),
                        Ok(None) => {
                            if parser_dummy_detected.load(Ordering::Relaxed) {
                                log::warn!(
                                    "kotatsu parser DUMMY detected for {}; terminating subprocess and switching to fallback",
                                    url
                                );
                                let _ = child.kill().await;
                                let _ = child.wait().await;
                                break Ok(None);
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        }
                        Err(e) => break Err(e),
                    }
                };

                match wait_status {
                    Ok(status_opt) => {
                        let exit_ok = status_opt.map(|s| s.success()).unwrap_or(false);
                        let mut effective_ok = exit_ok;

                        // For sources that often fail with kotatsu parser DUMMY, attempt internal fallback
                        // when process failed or when no recent output was produced.
                        let url_l = url.to_lowercase();
                        let should_try_fallback = url_l.contains("niadd") || url_l.contains("mangalivre");
                        if should_try_fallback {
                            let no_recent_output = !has_recent_download_content(&per_manga_dest, start_time);
                            if !exit_ok || no_recent_output {
                                log::warn!(
                                    "kotatsu produced no usable output for {} (exit_ok={}, no_recent_output={}); trying internal fallback",
                                    url,
                                    exit_ok,
                                    no_recent_output
                                );
                                let fallback_result = if url_l.contains("mangalivre") {
                                    internal_download_mangalivre(&url, &per_manga_dest, &chapters, &format, &download_id_clone, &pool_clone, start_time).await
                                } else {
                                    internal_download_niadd(&url, &per_manga_dest, &chapters, &format, &download_id_clone, &pool_clone, start_time).await
                                };

                                match fallback_result {
                                    Ok(_) => {
                                        effective_ok = true;
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "internal fallback download failed for {}: {}",
                                            url,
                                            e
                                        );
                                        effective_ok = false;
                                        let mut map = DOWNLOADS.lock().await;
                                        if let Some(entry) = map.get_mut(&download_id_clone) {
                                            let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                                            obj.insert("error".to_string(), serde_json::Value::String(e.to_string()));
                                            *entry = serde_json::Value::Object(obj);
                                        }
                                    }
                                }
                            }
                        }

                        let final_status = if effective_ok { "completed" } else { "failed" };
                        let mut map = DOWNLOADS.lock().await;
                        if let Some(mut entry) = map.get_mut(&download_id_clone) {
                            let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                            obj.insert("status".to_string(), serde_json::Value::String(final_status.to_string()));
                            obj.insert("progress".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
                            *entry = serde_json::Value::Object(obj);
                        }

                        // Import only the manga root folder (never chapter subfolders/files).
                        if effective_ok {
                            if let Some(ref pool_actual) = pool_clone {
                                let root_index = per_manga_dest.join("index.json");
                                if root_index.exists() {
                                    match LibraryService::parse_manga_from_index(&root_index).await {
                                        Ok(manga) => {
                                            if let Err(err) = MangaRepository::upsert_manga(pool_actual, &manga).await {
                                                log::error!("Failed to upsert manga from root index: {}", err);
                                            } else {
                                                log::info!("Imported downloaded manga to library: {}", manga.title);
                                            }
                                        }
                                        Err(err) => {
                                            log::warn!("Failed to parse root index.json for {}: {}", per_manga_dest.display(), err);
                                        }
                                    }
                                } else {
                                    let title = per_manga_dest
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let cover_path = find_cover_in_downloaded_tree(&per_manga_dest);
                                    let chapter_count = count_downloaded_chapters_in_tree(&per_manga_dest);
                                    let stable_id = format!(
                                        "manual_{}",
                                        per_manga_dest
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("unknown")
                                    );
                                    let manga = crate::library::library_service::Manga {
                                        id: stable_id,
                                        title: title.clone(),
                                        source_id: "manual".to_string(),
                                        source_name: "Manual".to_string(),
                                        cover_path,
                                        synopsis: None,
                                        status: "unknown".to_string(),
                                        rating: 0.0,
                                        language: "Português".to_string(),
                                        local_path: per_manga_dest.to_string_lossy().to_string(),
                                        total_chapters: chapter_count,
                                        downloaded_chapters: chapter_count,
                                        last_updated: chrono::Local::now().to_rfc3339(),
                                    };
                                    if let Err(err) = MangaRepository::upsert_manga(pool_actual, &manga).await {
                                        log::error!("Failed to upsert root placeholder manga: {}", err);
                                    } else {
                                        log::info!("Imported root downloaded folder to library: {}", manga.title);
                                    }
                                }
                            } else {
                                log::warn!("DB pool not ready; skipping import of root folder {}", per_manga_dest.display());
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("kotatsu wait failed: {}", e);
                        let mut map = DOWNLOADS.lock().await;
                        map.insert(download_id_clone.clone(), json!({"status": "error", "error": e.to_string()}));
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to spawn kotatsu process: {}", e);
                let mut map = DOWNLOADS.lock().await;
                map.insert(download_id_clone.clone(), json!({"status": "error", "error": e.to_string()}));
            }
        }
    });

    Ok(json!({"download_id": download_id, "status": "running", "dest": dest.to_string_lossy().to_string()}))
}

#[tauri::command]
pub async fn list_downloads() -> Result<serde_json::Value, String> {
    log::info!("Listing downloads");
    let mut map = DOWNLOADS.lock().await;
    // Enrich entries missing title/coverPath by inspecting dest folder
    for (_id, v) in map.iter_mut() {
        if let Some(obj) = v.as_object_mut() {
            if (!obj.contains_key("title") || !obj.contains_key("coverPath")) && obj.contains_key("dest") {
                let dest_opt = obj.get("dest").and_then(|d| d.as_str()).map(|s| s.to_string());
                if let Some(dest_val) = dest_opt {
                    let p = std::path::Path::new(&dest_val);
                    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                        obj.insert("title".to_string(), serde_json::Value::String(name.to_string()));
                    }
                    // try common cover names
                    if let Ok(entries) = std::fs::read_dir(p) {
                        for e in entries.flatten() {
                            if let Ok(ft) = e.file_type() {
                                if ft.is_file() {
                                    if let Some(fname) = e.file_name().to_str() {
                                        let low = fname.to_lowercase();
                                        if low == "cover.jpg" || low == "cover.png" || low == "cover.webp" {
                                            obj.insert("coverPath".to_string(), serde_json::Value::String(e.path().to_string_lossy().to_string()));
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

    let items: Vec<_> = map.iter().map(|(k,v)| json!({"id": k, "info": v})).collect();
    Ok(json!({"downloads": items}))
}

#[tauri::command]
pub async fn get_download_progress(download_id: String) -> Result<serde_json::Value, String> {
    log::info!("Getting progress for download: {}", download_id);
    let map = DOWNLOADS.lock().await;
    if let Some(v) = map.get(&download_id) {
        Ok(json!(v))
    } else {
        Err("download_id not found".to_string())
    }
}

#[tauri::command]
pub async fn remove_download(download_id: String) -> Result<serde_json::Value, String> {
    log::info!("Removing download: {}", download_id);
    let mut map = DOWNLOADS.lock().await;
    if let Some(entry) = map.remove(&download_id) {
        // Try to delete dest folder if present
        if let Some(dest) = entry.get("dest").and_then(|d| d.as_str()) {
            let p = std::path::Path::new(dest);
            if p.exists() {
                // attempt remove file or dir
                if p.is_file() {
                    if let Err(e) = std::fs::remove_file(p) {
                        log::warn!("Failed to remove downloaded file {}: {}", dest, e);
                    }
                } else if p.is_dir() {
                    if let Err(e) = std::fs::remove_dir_all(p) {
                        log::warn!("Failed to remove downloaded folder {}: {}", dest, e);
                    }
                }
                // Also try to remove any library entry that points to this path
                if let Ok(pool) = crate::config::database::get_connection_pool().await {
                    let target = dest.to_string();
                    match sqlx::query!("DELETE FROM manga WHERE local_path = ?", target).execute(&pool).await {
                        Ok(r) => {
                            let affected = r.rows_affected();
                            log::info!("Deleted {} library rows for removed download", affected);
                        }
                        Err(e) => log::warn!("Failed to delete library rows for removed download {}: {}", dest, e),
                    }
                }
            }
        }
        return Ok(json!({"status":"removed","download_id": download_id}));
    }
    Err("download_id not found".to_string())
}

#[tauri::command]
pub async fn list_downloaded_items() -> Result<serde_json::Value, String> {
    log::info!("Listing downloaded items from DB");
    let pool = match crate::config::database::get_connection_pool().await {
        Ok(p) => p,
        Err(e) => return Err(format!("DB pool error: {}", e)),
    };

    let rows = match sqlx::query(
        "SELECT id, title, cover_path, local_path, COALESCE(last_updated, created_at) AS sort_ts
         FROM manga
         WHERE source_id IN ('local','manual')
         ORDER BY sort_ts DESC"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return Err(format!("DB query failed: {}", e)),
    };

    let mut by_local: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();

    for r in rows {
        let local_path = r.get::<String, _>("local_path");
        if local_path.trim().is_empty() {
            continue;
        }

        let row_id = r.get::<String, _>("id");
        let row_title = r.get::<String, _>("title");
        let row_cover = r.get::<Option<String>, _>("cover_path");

        let fallback_title = std::path::Path::new(&local_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Manual Download")
            .replace('_', " ");

        let entry = by_local.entry(local_path.clone()).or_insert_with(|| {
            json!({
                "id": row_id,
                "title": if row_title.trim().is_empty() { fallback_title.clone() } else { row_title.clone() },
                "coverPath": row_cover,
                "localPath": local_path,
            })
        });

        if let Some(obj) = entry.as_object_mut() {
            let has_title = obj
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| !s.trim().is_empty() && s != "Manual Download")
                .unwrap_or(false);

            if !has_title && !row_title.trim().is_empty() {
                obj.insert("title".to_string(), serde_json::Value::String(row_title));
            }

            let has_cover = obj
                .get("coverPath")
                .and_then(|v| v.as_str())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

            if !has_cover {
                if let Some(cp) = row_cover {
                    if !cp.trim().is_empty() {
                        obj.insert("coverPath".to_string(), serde_json::Value::String(cp));
                    }
                }
            }
        }
    }

    let items: Vec<serde_json::Value> = by_local.into_values().collect();

    Ok(json!({"items": items}))
}

#[tauri::command]
pub async fn remove_downloaded_manga(manga_id: String) -> Result<serde_json::Value, String> {
    log::info!("Removing downloaded manga from DB: {}", manga_id);
    let pool = match crate::config::database::get_connection_pool().await {
        Ok(p) => p,
        Err(e) => return Err(format!("DB pool error: {}", e)),
    };

    // fetch local_path
    match sqlx::query!("SELECT local_path FROM manga WHERE id = ?", manga_id).fetch_optional(&pool).await {
        Ok(opt) => {
            if let Some(r) = opt {
                let local = r.local_path;
                if !local.is_empty() {
                    let p = std::path::Path::new(&local);
                    if p.exists() {
                        if p.is_file() {
                            if let Err(e) = std::fs::remove_file(p) {
                                log::warn!("Failed to remove file {}: {}", local, e);
                            }
                        } else if p.is_dir() {
                            if let Err(e) = std::fs::remove_dir_all(p) {
                                log::warn!("Failed to remove dir {}: {}", local, e);
                            }
                        }
                    }
                }
                // delete DB row
                match sqlx::query!("DELETE FROM manga WHERE id = ?", manga_id).execute(&pool).await {
                    Ok(r) => {
                        let affected = r.rows_affected();
                        log::info!("Deleted {} DB rows for manga {}", affected, manga_id);
                    }
                    Err(e) => log::warn!("Failed to delete manga {}: {}", manga_id, e),
                }
                // UI will reload after command returns; no emit here.
                return Ok(json!({"status":"removed","manga_id": manga_id}));
            }
            Err("manga_id not found".to_string())
        }
        Err(e) => Err(format!("DB query failed: {}", e)),
    }
}

async fn internal_download_niadd(
    url: &str,
    per_manga_dest: &std::path::Path,
    chapters: &str,
    format: &str,
    download_id: &str,
    pool_opt: &Option<sqlx::SqlitePool>,
    start_time: SystemTime,
) -> Result<(), anyhow::Error> {
    log::info!("Internal Niadd downloader started for {}", url);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    // Ensure per_manga_dest exists
    if !per_manga_dest.exists() {
        fs::create_dir_all(per_manga_dest).await.ok();
    }

    // Fetch manga page
    let resp = client.get(url).send().await?;
    let html = resp.text().await?;
    // Parse HTML and extract chapter links in a tight scope so the
    // non-Send `scraper::Html` value is dropped before any `.await`.
    let chapter_links: Vec<String> = {
        let doc = scraper::Html::parse_document(&html);
        let chapter_selectors = vec!["a.chapter-item", "ul.chapters a", "a[href*='chapter']", "a[href*='capitulo']", "a[href*='/c/']"];
        let mut chapter_links: Vec<String> = Vec::new();
        for sel in chapter_selectors {
            if let Ok(selector) = scraper::Selector::parse(sel) {
                for el in doc.select(&selector) {
                    if let Some(h) = el.value().attr("href") {
                        chapter_links.push(h.to_string());
                    }
                }
                if !chapter_links.is_empty() { break; }
            }
        }

        if chapter_links.is_empty() {
            if let Ok(sel) = scraper::Selector::parse("a[href]") {
                for el in doc.select(&sel) {
                    if let Some(h) = el.value().attr("href") {
                        if h.contains("chapter") || h.contains("cap") {
                            chapter_links.push(h.to_string());
                        }
                    }
                }
            }
        }

        chapter_links
    };

    if chapter_links.is_empty() {
        return Err(anyhow::anyhow!("No chapter links found on Niadd page"));
    }

    // Resolve absolute URLs
    let base = Url::parse(url)?;
    let mut abs_links: Vec<String> = Vec::new();
    for l in chapter_links.iter() {
        if let Ok(u) = Url::parse(l) {
            abs_links.push(u.to_string());
        } else if let Ok(u) = base.join(l) {
            abs_links.push(u.to_string());
        }
    }

    // Pick first requested chapter only (simple implementation) and compute chapter index
    let (chapter_url, chapter_index): (String, usize) = if chapters.trim().is_empty() || chapters.trim() == "all" {
        (abs_links.first().cloned().ok_or_else(|| anyhow::anyhow!("No chapters available"))?, 1usize)
    } else {
        // Try parse a single number like "1" or a range "1-3" -> take first
        let first_token = chapters.split(&[',',';'][..]).next().unwrap_or(chapters).trim();
        if let Some(idx_dash) = first_token.find('-') {
            let n = first_token[..idx_dash].trim().parse::<usize>().unwrap_or(1);
            let url = abs_links.get(n.saturating_sub(1)).cloned().unwrap_or_else(|| abs_links[0].clone());
            (url, n)
        } else if let Ok(n) = first_token.parse::<usize>() {
            let url = abs_links.get(n.saturating_sub(1)).cloned().unwrap_or_else(|| abs_links[0].clone());
            (url, n)
        } else {
            (abs_links[0].clone(), 1usize)
        }
    };

    log::info!("Niadd: selected chapter URL {} (index {})", chapter_url, chapter_index);

    // Fetch chapter page
    let ch_resp = client.get(&chapter_url).send().await?;
    let ch_html = ch_resp.text().await?;
    // Parse chapter HTML and extract image URLs in a tight scope so the
    // `scraper::Html` value doesn't live across async awaits.
    let mut images: Vec<String> = {
        let ch_doc = scraper::Html::parse_document(&ch_html);
        let img_selectors = vec!["img.page-img", "img#image", "div.reading-content img", "img"];
        let mut images: Vec<String> = Vec::new();
        for s in img_selectors {
            if let Ok(sel) = scraper::Selector::parse(s) {
                for el in ch_doc.select(&sel) {
                    if let Some(src) = el.value().attr("data-src").or_else(|| el.value().attr("src")) {
                        images.push(src.to_string());
                    }
                }
                if !images.is_empty() { break; }
            }
        }
        images
    };

    if images.is_empty() {
        return Err(anyhow::anyhow!("No images found in chapter page"));
    }

    // Filter images to likely page images (extensions and avoid brand/logo assets)
    let original_images = images.clone();
    images = images.into_iter().filter(|u| {
        let low = u.to_lowercase();
        (low.ends_with(".jpg") || low.ends_with(".jpeg") || low.ends_with(".png") || low.ends_with(".webp"))
            && !low.contains("brand") && !low.contains("logo") && !low.contains("avatar") && !low.contains("favicon")
    }).collect();
    if images.is_empty() {
        // if filtering removed everything, fall back to original list
        images = original_images;
    }

    log::info!("Niadd: found {} image candidates", images.len());

    // Download images sequentially and save (resilient to single-image failures)
    let chapter_dir = per_manga_dest.join(format!("chapter_{}", chapter_index));
    fs::create_dir_all(&chapter_dir).await?;
    let total = images.len();
    let mut success_count: usize = 0;
    for (i, img_url) in images.iter().enumerate() {
        let img_abs = if let Ok(u) = Url::parse(img_url) { u.to_string() } else { base.join(img_url)?.to_string() };
        log::info!("Downloading image {}", img_abs);
        let filename = format!("{:03}.jpg", i+1);
        let path = chapter_dir.join(&filename);

        match client.get(&img_abs).send().await {
            Ok(resp) => match resp.bytes().await {
                Ok(b) => {
                    if let Err(e) = fs::write(&path, &b).await {
                        log::warn!("Failed to write image {}: {}", path.display(), e);
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    log::warn!("Failed to read bytes for {}: {}", img_abs, e);
                }
            },
            Err(e) => {
                log::warn!("Failed to download {}: {}", img_abs, e);
            }
        }

        // update progress even on failures
        let mut map = DOWNLOADS.lock().await;
        if let Some(mut entry) = map.get_mut(download_id) {
            let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
            let pct = ((i+1) * 100 / total) as u8;
            obj.insert("last_stdout".to_string(), serde_json::Value::String(format!("Attempted {}", filename)));
            obj.insert("progress".to_string(), serde_json::Value::Number(serde_json::Number::from(pct)));
            obj.insert("downloaded".to_string(), serde_json::Value::Number(serde_json::Number::from(success_count as u64)));
            *entry = serde_json::Value::Object(obj);
        }
    }

    log::info!("Niadd: downloaded {}/{} images", success_count, total);

    // If format == cbz, create zip
    if format == "cbz" {
        let cbz_path = per_manga_dest.join(format!("chapter_{}.cbz", chapter_index));
        let chapter_dir2 = chapter_dir.clone();
        let cbz_path_for_task = cbz_path.clone();
        log::info!("Niadd: creating CBZ at {}", cbz_path.to_string_lossy());
        // spawn blocking for zip creation
        let zip_result = tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let file = std::fs::File::create(&cbz_path_for_task)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for entry in std::fs::read_dir(&chapter_dir2)? {
                let e = entry?;
                if e.path().is_file() {
                    let name = e.file_name().into_string().unwrap_or_else(|_| "img".to_string());
                    zip.start_file(name, options)?;
                    let mut f = std::fs::File::open(e.path())?;
                    let mut buf = Vec::new();
                    use std::io::Read;
                    f.read_to_end(&mut buf)?;
                    zip.write_all(&buf)?;
                }
            }
            zip.finish()?;
            Ok(())
        }).await;

        match zip_result {
            Ok(Ok(())) => log::info!("Niadd: CBZ creation completed: {}", cbz_path.to_string_lossy()),
            Ok(Err(e)) => log::error!("Niadd: CBZ creation failed: {}", e),
            Err(e) => log::error!("Niadd: CBZ creation task panicked or was cancelled: {}", e),
        }
    }

    // mark completed
    log::info!("Niadd: marking download {} as completed", download_id);
    let mut map = DOWNLOADS.lock().await;
    if let Some(mut entry) = map.get_mut(download_id) {
        let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
        obj.insert("status".to_string(), serde_json::Value::String("completed".to_string()));
        obj.insert("progress".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
        obj.insert("dest".to_string(), serde_json::Value::String(per_manga_dest.to_string_lossy().to_string()));

        // Try to infer a title from folder name
        if let Some(name) = per_manga_dest.file_name().and_then(|s| s.to_str()) {
            obj.insert("title".to_string(), serde_json::Value::String(name.to_string()));
        }

        // Find a cover image if available (common names or first page image)
        let mut cover_path: Option<String> = None;
        if let Ok(mut entries) = std::fs::read_dir(&per_manga_dest) {
            for e in entries.flatten() {
                if let Ok(ft) = e.file_type() {
                    if ft.is_file() {
                        if let Some(fname) = e.file_name().to_str() {
                            let low = fname.to_lowercase();
                            if low == "cover.jpg" || low == "cover.png" || low == "cover.webp" {
                                cover_path = Some(e.path().to_string_lossy().to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        if cover_path.is_none() {
            if let Ok(entries) = std::fs::read_dir(&per_manga_dest) {
                'outer: for e in entries.flatten() {
                    if let Ok(ft) = e.file_type() {
                        if ft.is_dir() {
                            if let Ok(mut inner) = std::fs::read_dir(e.path()) {
                                for ie in inner.flatten() {
                                    if let Ok(ift) = ie.file_type() {
                                        if ift.is_file() {
                                            if let Some(iname) = ie.file_name().to_str() {
                                                let low = iname.to_lowercase();
                                                if low.ends_with(".jpg") || low.ends_with(".png") || low.ends_with(".webp") {
                                                    cover_path = Some(ie.path().to_string_lossy().to_string());
                                                    break 'outer;
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
        }

        if let Some(cp) = cover_path {
            obj.insert("coverPath".to_string(), serde_json::Value::String(cp));
        }

        *entry = serde_json::Value::Object(obj);
        log::info!("Niadd: download {} updated in DOWNLOADS map", download_id);
    } else {
        log::warn!("Niadd: download id {} not found in DOWNLOADS map when trying to mark completed", download_id);
    }

    // Optionally upsert into DB
    if let Some(pool_actual) = pool_opt {
        // write a placeholder Manga entry
        let stable_id = format!(
            "manual_{}",
            per_manga_dest
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
        );
        let manga = crate::library::library_service::Manga {
            id: stable_id,
            title: "Manual Download".to_string(),
            source_id: "manual".to_string(),
            source_name: "Manual".to_string(),
            cover_path: None,
            synopsis: None,
            status: "unknown".to_string(),
            rating: 0.0,
            language: "Português".to_string(),
            local_path: per_manga_dest.to_string_lossy().to_string(),
            total_chapters: 0,
            downloaded_chapters: 0,
            last_updated: chrono::Local::now().to_rfc3339(),
        };
        if let Err(e) = MangaRepository::upsert_manga(pool_actual, &manga).await {
            log::warn!("Failed to upsert downloaded manual manga: {}", e);
        }
    }

    Ok(())
}

fn parse_requested_chapters(chapters: &str, max_available: usize) -> Vec<usize> {
    let raw = chapters.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("all") {
        return (1..=max_available).collect();
    }

    let mut out: Vec<usize> = Vec::new();
    for token in raw.split(&[',', ';'][..]).map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Some((a, b)) = token.split_once('-') {
            let start = a.trim().parse::<usize>().unwrap_or(1);
            let end = b.trim().parse::<usize>().unwrap_or(start);
            let lo = start.min(end).max(1);
            let hi = start.max(end).min(max_available.max(1));
            for n in lo..=hi {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
            continue;
        }
        if let Ok(n) = token.parse::<usize>() {
            let n = n.max(1).min(max_available.max(1));
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }

    if out.is_empty() {
        vec![1]
    } else {
        out.sort_unstable();
        out
    }
}

fn chapter_number_from_url(url: &str) -> Option<f64> {
    let marker = "capitulo-";
    let low = url.to_lowercase();
    let idx = low.find(marker)? + marker.len();
    let tail = &low[idx..];
    let mut num = String::new();
    for ch in tail.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else {
            break;
        }
    }
    if num.is_empty() {
        None
    } else {
        num.parse::<f64>().ok()
    }
}

fn chapter_label_from_url(url: &str) -> Option<(String, String)> {
    let num = chapter_number_from_url(url)?;
    let display = if (num.fract()).abs() < f64::EPSILON {
        format!("{}", num as i64)
    } else {
        let mut s = format!("{:.3}", num);
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    };
    let file_label = display.replace('.', "_");
    Some((display, file_label))
}

fn extract_image_urls_from_text(raw: &str) -> Vec<String> {
    // Manga sites often embed page image URLs in script blobs as escaped strings.
    let normalized = raw.replace("\\/", "/");
    let mut urls: Vec<String> = normalized
        .split(|c: char| {
            c == '"'
                || c == '\''
                || c == ' '
                || c == '\n'
                || c == '\r'
                || c == '\t'
                || c == '<'
                || c == '>'
                || c == '('
                || c == ')'
                || c == ','
        })
        .map(|s| s.trim())
        .filter(|s| {
            let low = s.to_lowercase();
            (low.contains(".jpg")
                || low.contains(".jpeg")
                || low.contains(".png")
                || low.contains(".webp"))
                && (low.starts_with("http://") || low.starts_with("https://") || low.starts_with("//") || low.starts_with('/'))
        })
        .map(|s| s.trim_matches(|c: char| c == ']' || c == '[' || c == ';').to_string())
        .collect();

    urls.sort();
    urls.dedup();
    urls
}

fn extract_chapter_urls_from_text(raw: &str) -> Vec<String> {
    let normalized = raw.replace("\\/", "/");
    let mut urls: Vec<String> = normalized
        .split(|c: char| {
            c == '"'
                || c == '\''
                || c == ' '
                || c == '\n'
                || c == '\r'
                || c == '\t'
                || c == '<'
                || c == '>'
                || c == '('
                || c == ')'
                || c == ','
        })
        .map(|s| s.trim().trim_matches(|c: char| c == ']' || c == '[' || c == ';'))
        .filter(|s| {
            let low = s.to_lowercase();
            (low.starts_with("http://") || low.starts_with("https://") || low.starts_with("//") || low.starts_with('/'))
                && low.contains("/capitulo-")
        })
        .map(|s| s.to_string())
        .collect();

    urls.sort();
    urls.dedup();
    urls
}

fn text_looks_like_block_page(raw: &str) -> bool {
    let low = raw.to_lowercase();
    low.contains("sorry, you have been blocked")
        || low.contains("attention required")
        || low.contains("cloudflare")
        || low.contains("cf-challenge")
        || low.contains("/cdn-cgi/")
}

fn bytes_look_like_image(bytes: &[u8]) -> bool {
    // JPEG
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return true;
    }
    // PNG
    if bytes.len() >= 8
        && bytes[0] == 0x89
        && bytes[1] == b'P'
        && bytes[2] == b'N'
        && bytes[3] == b'G'
        && bytes[4] == 0x0D
        && bytes[5] == 0x0A
        && bytes[6] == 0x1A
        && bytes[7] == 0x0A
    {
        return true;
    }
    // WEBP: RIFF....WEBP
    if bytes.len() >= 12
        && bytes[0] == b'R'
        && bytes[1] == b'I'
        && bytes[2] == b'F'
        && bytes[3] == b'F'
        && bytes[8] == b'W'
        && bytes[9] == b'E'
        && bytes[10] == b'B'
        && bytes[11] == b'P'
    {
        return true;
    }
    // GIF
    if bytes.len() >= 6
        && bytes[0] == b'G'
        && bytes[1] == b'I'
        && bytes[2] == b'F'
        && bytes[3] == b'8'
        && (bytes[4] == b'7' || bytes[4] == b'9')
        && bytes[5] == b'a'
    {
        return true;
    }

    false
}

fn manga_mirror_url(url: &str) -> String {
    // r.jina.ai often bypasses anti-bot pages by returning rendered text content.
    format!("https://r.jina.ai/http://{}", url.trim_start_matches("https://").trim_start_matches("http://"))
}

async fn fetch_page_with_403_fallback(
    client: &reqwest::Client,
    url: &str,
) -> Result<(String, String), anyhow::Error> {
    let direct = client.get(url).send().await;
    match direct {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                return Ok((body, format!("direct {}", status)));
            }
            if status != reqwest::StatusCode::FORBIDDEN {
                return Err(anyhow::anyhow!(
                    "Direct fetch failed with status {}",
                    status
                ));
            }
            let mirror_url = manga_mirror_url(url);
            let mirror_resp = client.get(&mirror_url).send().await?;
            let mirror_status = mirror_resp.status();
            let mirror_body = mirror_resp.text().await.unwrap_or_default();
            if mirror_status.is_success() {
                Ok((mirror_body, format!("mirror {}", mirror_status)))
            } else {
                Err(anyhow::anyhow!(
                    "Direct status {} and mirror status {}",
                    status,
                    mirror_status
                ))
            }
        }
        Err(e) => {
            let mirror_url = manga_mirror_url(url);
            let mirror_resp = client.get(&mirror_url).send().await?;
            let mirror_status = mirror_resp.status();
            let mirror_body = mirror_resp.text().await.unwrap_or_default();
            if mirror_status.is_success() {
                Ok((mirror_body, format!("mirror {} after direct error {}", mirror_status, e)))
            } else {
                Err(anyhow::anyhow!(
                    "Direct fetch error {}; mirror status {}",
                    e,
                    mirror_status
                ))
            }
        }
    }
}

async fn internal_download_mangalivre(
    url: &str,
    per_manga_dest: &std::path::Path,
    chapters: &str,
    format: &str,
    download_id: &str,
    pool_opt: &Option<sqlx::SqlitePool>,
    _start_time: SystemTime,
) -> Result<(), anyhow::Error> {
    log::info!("Internal MangaLivre downloader started for {}", url);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(25))
        .build()?;

    fs::create_dir_all(per_manga_dest).await.ok();

    let manga_url = if url.contains("/capitulo-") {
        let idx = url.find("/capitulo-").unwrap_or(url.len());
        format!("{}{}", &url[..idx], "/")
    } else {
        url.to_string()
    };

    let (manga_html, manga_fetch_info) = fetch_page_with_403_fallback(&client, &manga_url).await?;
    log::info!(
        "MangaLivre page fetched {} via {}",
        manga_url,
        manga_fetch_info
    );
    if text_looks_like_block_page(&manga_html) {
        return Err(anyhow::anyhow!(
            "MangaLivre blocked the manga page request (Cloudflare/anti-bot)"
        ));
    }
    let mut chapter_links: Vec<String> = {
        let doc = scraper::Html::parse_document(&manga_html);
        let mut links = Vec::new();
        let chapter_selectors = [
            // MangaLivre layout used by emi_scraper
            "ul.full-chapters-list.list-of-chapters a[href]",
            "ul.list-of-chapters a[href]",
            // Generic fallbacks
            "a[href*='/capitulo-']",
        ];
        for sel_txt in chapter_selectors {
            if let Ok(sel) = scraper::Selector::parse(sel_txt) {
                for el in doc.select(&sel) {
                    if let Some(h) = el.value().attr("href") {
                        links.push(h.to_string());
                    }
                }
                if !links.is_empty() {
                    break;
                }
            }
        }
        links
    };

    if chapter_links.is_empty() {
        let from_text = extract_chapter_urls_from_text(&manga_html);
        if !from_text.is_empty() {
            log::info!(
                "MangaLivre chapter links extracted from text/script: {}",
                from_text.len()
            );
            chapter_links = from_text;
        }
    }

    log::info!("MangaLivre chapter link candidates: {}", chapter_links.len());

    if chapter_links.is_empty() {
        return Err(anyhow::anyhow!("No chapter links found on MangaLivre page"));
    }

    let base = Url::parse(&manga_url)?;
    let mut abs_links: Vec<String> = chapter_links
        .iter()
        .filter_map(|l| {
            if let Ok(u) = Url::parse(l) {
                Some(u.to_string())
            } else {
                base.join(l).ok().map(|u| u.to_string())
            }
        })
        .collect();

    abs_links.sort_by(|a, b| {
        let na = chapter_number_from_url(a).unwrap_or(0.0);
        let nb = chapter_number_from_url(b).unwrap_or(0.0);
        na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
    });
    abs_links.dedup();
    log::info!("MangaLivre chapter links after normalization: {}", abs_links.len());

    let selected = parse_requested_chapters(chapters, abs_links.len());
    if selected.is_empty() {
        return Err(anyhow::anyhow!("No chapters selected from requested range"));
    }
    log::info!(
        "MangaLivre selected chapter indexes {:?} ({} available)",
        selected,
        abs_links.len()
    );

    let total_chapters = selected.len().max(1);
    let mut total_saved_pages: usize = 0;
    let mut total_chapters_with_pages: usize = 0;
    for (sel_idx, chapter_idx) in selected.iter().enumerate() {
        let chapter_url = abs_links
            .get(chapter_idx.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| abs_links[0].clone());
        let (chapter_display, chapter_file_label) = chapter_label_from_url(&chapter_url)
            .unwrap_or_else(|| (chapter_idx.to_string(), chapter_idx.to_string()));
        log::info!(
            "MangaLivre downloading chapter {} (requested index {}) -> {}",
            chapter_display,
            chapter_idx,
            chapter_url
        );

        let (ch_html, chapter_fetch_info) = fetch_page_with_403_fallback(&client, &chapter_url).await?;
        log::info!(
            "MangaLivre chapter page fetched {} via {}",
            chapter_url,
            chapter_fetch_info
        );
        if text_looks_like_block_page(&ch_html) {
            log::warn!(
                "MangaLivre chapter {} appears blocked by anti-bot page",
                chapter_display
            );
            continue;
        }

        let chapter_base = Url::parse(&chapter_url).unwrap_or_else(|_| base.clone());
        let mut images: Vec<String> = {
            let ch_doc = scraper::Html::parse_document(&ch_html);
            let mut list = Vec::new();
            let selectors = vec![
                // MangaLivre structure used by emi_scraper
                "div.manga-image picture img",
                "div.manga-continue img",
                "img.wp-manga-chapter-img",
                ".reading-content img",
                ".entry-content img",
                "img[data-src]",
                "img[src]",
            ];
            for s in selectors {
                if let Ok(sel) = scraper::Selector::parse(s) {
                    for el in ch_doc.select(&sel) {
                        if let Some(src) = el.value().attr("data-src").or_else(|| el.value().attr("src")) {
                            list.push(src.to_string());
                        }
                    }
                }
                if !list.is_empty() {
                    break;
                }
            }
            list
        };

        if images.is_empty() {
            let from_scripts = extract_image_urls_from_text(&ch_html);
            if !from_scripts.is_empty() {
                log::info!(
                    "MangaLivre chapter {} extracted {} image candidates from scripts/text",
                    chapter_display,
                    from_scripts.len()
                );
                images = from_scripts;
            }
        }

        images = images
            .into_iter()
            .filter(|u| {
                let low = u.to_lowercase();
                (low.ends_with(".jpg")
                    || low.ends_with(".jpeg")
                    || low.ends_with(".png")
                    || low.ends_with(".webp")
                    || low.contains(".jpg?")
                    || low.contains(".png?")
                    || low.contains(".webp?"))
                    && !low.starts_with("data:")
                    && !low.contains("logo")
                    && !low.contains("avatar")
                    && !low.contains("favicon")
            })
            .collect();

        if images.is_empty() {
            log::warn!("MangaLivre chapter {} has no image candidates", chapter_display);
            let mut map = DOWNLOADS.lock().await;
            if let Some(entry) = map.get_mut(download_id) {
                let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                obj.insert(
                    "last_stdout".to_string(),
                    serde_json::Value::String(format!(
                        "Chapter {} has no pages",
                        chapter_display
                    )),
                );
                *entry = serde_json::Value::Object(obj);
            }
            continue;
        }

        let chapter_dir = per_manga_dest.join(format!("chapter_{}", chapter_file_label));
        fs::create_dir_all(&chapter_dir).await?;
        let mut chapter_saved_pages: usize = 0;
        for (img_i, img_url) in images.iter().enumerate() {
            let img_abs = if let Ok(u) = Url::parse(img_url) {
                u.to_string()
            } else if let Ok(u) = chapter_base.join(img_url) {
                u.to_string()
            } else {
                continue;
            };

            let ext = if img_abs.to_lowercase().contains(".webp") {
                "webp"
            } else if img_abs.to_lowercase().contains(".png") {
                "png"
            } else {
                "jpg"
            };
            let path = chapter_dir.join(format!("{:03}.{}", img_i + 1, ext));

            if let Ok(resp) = client
                .get(&img_abs)
                .header("Referer", &chapter_url)
                .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
                .send()
                .await
            {
                let status_ok = resp.status().is_success();
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                if let Ok(bytes) = resp.bytes().await {
                    let body_as_text = std::str::from_utf8(&bytes).ok().unwrap_or("");
                    let body_blocked = text_looks_like_block_page(body_as_text);
                    let content_type_is_image = content_type.starts_with("image/");
                    let body_is_image = bytes_look_like_image(&bytes);

                    if status_ok && !body_blocked && (content_type_is_image || body_is_image) {
                        if fs::write(&path, &bytes).await.is_ok() {
                            chapter_saved_pages += 1;
                        }
                    } else {
                        log::warn!(
                            "Skipping invalid MangaLivre image response for chapter {} page {} (status_ok={}, content_type='{}', blocked={}, image_magic={})",
                            chapter_display,
                            img_i + 1,
                            status_ok,
                            content_type,
                            body_blocked,
                            body_is_image
                        );
                    }
                }
            }

            let chapter_pct = ((img_i + 1) * 100 / images.len()) as u32;
            let global_pct = ((sel_idx as u32) * 100 / total_chapters as u32)
                + (chapter_pct / total_chapters as u32);
            let mut map = DOWNLOADS.lock().await;
            if let Some(entry) = map.get_mut(download_id) {
                let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
                obj.insert(
                    "last_stdout".to_string(),
                    serde_json::Value::String(format!("Chapter {} page {}", chapter_display, img_i + 1)),
                );
                obj.insert(
                    "progress".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(global_pct.min(99))),
                );
                *entry = serde_json::Value::Object(obj);
            }
        }

        if chapter_saved_pages == 0 {
            log::warn!(
                "MangaLivre chapter {} had image candidates but no pages were saved",
                chapter_display
            );
        } else {
            total_saved_pages += chapter_saved_pages;
            total_chapters_with_pages += 1;
            log::info!(
                "MangaLivre chapter {} saved {} pages",
                chapter_display,
                chapter_saved_pages
            );
        }

        if format == "cbz" && chapter_saved_pages > 0 {
            let chapter_dir2 = chapter_dir.clone();
            let cbz_path = per_manga_dest.join(format!("chapter_{}.cbz", chapter_file_label));
            let cbz_path_for_task = cbz_path.clone();
            let _ = tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
                let file = std::fs::File::create(&cbz_path_for_task)?;
                let mut zip = zip::ZipWriter::new(file);
                let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
                for entry in std::fs::read_dir(&chapter_dir2)? {
                    let e = entry?;
                    if e.path().is_file() {
                        let name = e.file_name().into_string().unwrap_or_else(|_| "img".to_string());
                        zip.start_file(name, options)?;
                        let mut f = std::fs::File::open(e.path())?;
                        let mut buf = Vec::new();
                        use std::io::Read;
                        f.read_to_end(&mut buf)?;
                        zip.write_all(&buf)?;
                    }
                }
                zip.finish()?;
                Ok(())
            })
            .await;
        }
    }

    if total_saved_pages == 0 {
        return Err(anyhow::anyhow!(
            "No pages were downloaded from MangaLivre ({} selected chapters)",
            selected.len()
        ));
    }

    log::info!(
        "MangaLivre fallback completed: {} pages across {} chapters",
        total_saved_pages,
        total_chapters_with_pages
    );

    let mut cover_path = find_cover_in_downloaded_tree(per_manga_dest);
    if cover_path.is_none() {
        let title_hint = per_manga_dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Manual Download")
            .replace('_', " ");
        cover_path = try_fetch_cover_by_title_to_local(&title_hint, per_manga_dest).await;
        if cover_path.is_some() {
            log::info!("Manual MangaLivre cover fetched from web for '{}'", title_hint);
        }
    }

    let mut map = DOWNLOADS.lock().await;
    if let Some(entry) = map.get_mut(download_id) {
        let mut obj = entry.as_object_mut().cloned().unwrap_or_default();
        obj.insert("status".to_string(), serde_json::Value::String("completed".to_string()));
        obj.insert("progress".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
        obj.insert(
            "dest".to_string(),
            serde_json::Value::String(per_manga_dest.to_string_lossy().to_string()),
        );
        if let Some(name) = per_manga_dest.file_name().and_then(|s| s.to_str()) {
            obj.insert("title".to_string(), serde_json::Value::String(name.to_string()));
        }
        if let Some(cp) = &cover_path {
            obj.insert("coverPath".to_string(), serde_json::Value::String(cp.clone()));
        }
        *entry = serde_json::Value::Object(obj);
    }

    if let Some(pool_actual) = pool_opt {
        let stable_id = format!(
            "manual_{}",
            per_manga_dest
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
        );
        let manga = crate::library::library_service::Manga {
            id: stable_id,
            title: per_manga_dest
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Manual Download")
                .to_string(),
            source_id: "manual".to_string(),
            source_name: "Manual".to_string(),
            cover_path: cover_path.clone(),
            synopsis: None,
            status: "unknown".to_string(),
            rating: 0.0,
            language: "Português".to_string(),
            local_path: per_manga_dest.to_string_lossy().to_string(),
            total_chapters: 0,
            downloaded_chapters: 0,
            last_updated: chrono::Local::now().to_rfc3339(),
        };
        let _ = MangaRepository::upsert_manga(pool_actual, &manga).await;
    }

    Ok(())
}
