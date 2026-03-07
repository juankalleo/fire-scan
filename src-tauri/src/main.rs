// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod library;
mod download;
mod reader;
mod search;
mod scraper;

use tauri::Manager;
use commands::library::AppState;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Initialize file + stdout logging using fern. Fall back to env_logger if it fails.
    match init_logging() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to initialize file logger: {}. Falling back to env_logger.", e);
            env_logger::Builder::from_default_env()
                .filter_level(log::LevelFilter::Info)
                .init();
        }
    }

    log::info!("FireScan starting...");

    // Create a lightweight placeholder AppState so commands can be invoked
    // before the async initialization completes. We'll populate the real
    // `db_pool` once the async initialization finishes.
    let placeholder_library = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let placeholder_state = AppState {
        db_pool: None,
        library_path: placeholder_library,
    };

    tauri::Builder::default()
        .manage(tokio::sync::Mutex::new(placeholder_state))
        .setup(|app| {
            log::info!("[SETUP] Starting application setup");
            
            // Initialize app directories
            log::info!("[SETUP] Initializing app paths...");
            match config::init::initialize_app_paths() {
                Ok(_) => log::info!("[SETUP] ✓ App paths initialized"),
                Err(e) => {
                    log::error!("[SETUP] ✗ Failed to initialize app paths: {}", e);
                    eprintln!("ERROR: Failed to initialize app paths: {}", e);
                }
            }

            // Use block_in_place for async work in sync context (setup hook is NOT async)
            // Try to detect an existing tokio runtime; if none exists, we'll still
            // perform the async initialization by creating a runtime inside a
            // background thread. This ensures the app initializes its DB and
            // library scan whether started via Tauri or directly as an executable.
            let handle = tokio::runtime::Handle::try_current();
            if handle.is_err() {
                log::warn!("[SETUP] No tokio runtime found in setup hook; async initialization will run on a dedicated runtime thread");
            }

            log::info!("[SETUP] Scheduling database initialization...");

            // Create a simple synchronous wrapper for critical initialization
            match config::init::get_library_path() {
                Ok(library_path) => {
                    log::info!("[SETUP] ✓ Library path: {:?}", library_path);
                    
                    // Try to create a temporary app state just to verify paths work
                    log::info!("[SETUP] Verifying library path exists...");
                    if library_path.exists() {
                        log::info!("[SETUP] ✓ Library path is accessible");
                    } else {
                        log::warn!("[SETUP] Library path doesn't exist yet (will be created on first use)");
                    }
                }
                Err(e) => {
                    log::error!("[SETUP] ✗ Failed to get library path: {}", e);
                }
            }

            // For async initialization, we spawn it in the background
            let app_handle = app.handle();
            std::thread::spawn(move || {
                log::info!("[ASYNC-INIT] Starting async initialization in background thread...");
                // Create a new runtime for background tasks
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(async {
                        log::info!("[ASYNC-INIT] Running database migrations...");
                        
                        // Run migrations
                        if let Err(e) = config::database::run_migrations().await {
                            log::error!("[ASYNC-INIT] ✗ Failed to run migrations: {}", e);
                            eprintln!("ERROR: Migrations failed: {}", e);
                            return;
                        }
                        log::info!("[ASYNC-INIT] ✓ Migrations completed");

                        // Get database pool
                        let pool = match config::database::get_connection_pool().await {
                            Ok(p) => {
                                log::info!("[ASYNC-INIT] ✓ Database pool created");
                                p
                            }
                            Err(e) => {
                                log::error!("[ASYNC-INIT] ✗ Failed to get db pool: {}", e);
                                eprintln!("ERROR: Database pool failed: {}", e);
                                return;
                            }
                        };

                        // Probe Niadd connectivity once at startup to help diagnostics
                        log::info!("[ASYNC-INIT] Probing Niadd connectivity...");
                        match async {
                            let client = reqwest::Client::builder()
                                .timeout(std::time::Duration::from_secs(10))
                                .build()?;
                            // Use the same host used by runtime scraper to avoid false negatives.
                            let url = "https://br.niadd.com/list/Hot-Manga/";
                            let resp = client.get(url).send().await?;
                            let status = resp.status();
                            let bytes = resp.bytes().await?;
                            let len = bytes.len();
                            // try parse
                            let parsed = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
                            Ok::<_, anyhow::Error>((url.to_string(), status, len, parsed))
                        }.await {
                            Ok((url, status, len, Some(json))) => {
                                let count = json.get("results").and_then(|r| r.as_array()).map(|a| a.len());
                                log::info!("[ASYNC-INIT] Niadd probe OK: {} status={} size={} parsed_results={:?}", url, status, len, count);
                            }
                            Ok((url, status, len, None)) => {
                                log::info!("[ASYNC-INIT] Niadd probe OK: {} status={} size={} (non-JSON HTML expected)", url, status, len);
                            }
                            Err(e) => {
                                log::warn!("[ASYNC-INIT] Niadd probe failed (non-fatal): {}", e);
                            }
                        }

                        // Get library path
                        let library_path = match config::init::get_library_path() {
                            Ok(p) => {
                                log::info!("[ASYNC-INIT] ✓ Library path: {:?}", p);
                                p
                            }
                            Err(e) => {
                                log::error!("[ASYNC-INIT] ✗ Failed to get library path: {}", e);
                                eprintln!("ERROR: Library path failed: {}", e);
                                return;
                            }
                        };

                        // Scan library directory
                        log::info!("[ASYNC-INIT] Scanning library directory...");
                        match library::LibraryService::scan_library(&library_path).await {
                            Ok(manga_list) => {
                                log::info!("[ASYNC-INIT] ✓ Found {} titles", manga_list.len());
                                for manga in manga_list {
                                    if let Err(e) = library::MangaRepository::upsert_manga(&pool, &manga).await {
                                        log::error!("[ASYNC-INIT] Failed to upsert manga {}: {}", manga.id, e);
                                    }
                                }
                                log::info!("[ASYNC-INIT] ✓ Library sync completed");
                            }
                            Err(e) => {
                                log::warn!("[ASYNC-INIT] Library scan failed (non-fatal): {}", e);
                            }
                        };

                        // Populate the managed placeholder AppState with the real pool/path
                        log::info!("[ASYNC-INIT] Populating managed AppState...");
                        let state_mutex = app_handle.state::<tokio::sync::Mutex<AppState>>();
                        let mut s = state_mutex.lock().await;
                        s.db_pool = Some(pool);
                        s.library_path = library_path.clone();
                        log::info!("[ASYNC-INIT] ✓ AppState updated in managed state");

                        // Attempt to run the fill_covers helper (if present) to auto-generate missing covers
                        // This is executed as a background process to avoid blocking startup.
                        if let Ok(mut exe_path) = std::env::current_exe() {
                            exe_path.pop(); // go to exe dir
                            let helper = exe_path.join("fill_covers.exe");
                            if helper.exists() {
                                log::info!("[ASYNC-INIT] Spawning fill_covers helper: {:?}", helper);
                                // spawn and don't wait
                                match Command::new(helper).spawn() {
                                    Ok(_) => log::info!("[ASYNC-INIT] fill_covers spawned"),
                                    Err(e) => log::warn!("[ASYNC-INIT] Failed to spawn fill_covers: {}", e),
                                }
                            } else {
                                log::warn!("[ASYNC-INIT] fill_covers helper not found at {:?}", helper);
                            }
                        } else {
                            log::warn!("[ASYNC-INIT] Could not determine current exe path to locate fill_covers helper");
                        }

                        // Start a periodic background task (runs immediately, then every 5 minutes)
                        if let Some(p) = &s.db_pool {
                            let pool_clone = p.clone();
                            let app_clone = app_handle.clone();
                            tokio::spawn(async move {
                                // run once immediately
                                match crate::commands::library::generate_missing_covers_startup(&pool_clone, app_clone.clone()).await {
                                    Ok(n) => log::info!("[COVERS-PERIODIC] initial generated {} covers", n),
                                    Err(e) => log::warn!("[COVERS-PERIODIC] initial generation failed: {}", e),
                                }

                                let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
                                loop {
                                    interval.tick().await;
                                    match crate::commands::library::generate_missing_covers_startup(&pool_clone, app_clone.clone()).await {
                                        Ok(n) => {
                                            if n > 0 { log::info!("[COVERS-PERIODIC] generated {} covers", n); }
                                        }
                                        Err(e) => log::warn!("[COVERS-PERIODIC] generation failed: {}", e),
                                    }
                                }
                            });
                        } else {
                            log::warn!("[ASYNC-INIT] DB pool missing; skipping periodic cover generation task");
                        }
                        // Emit initial library and downloads snapshot so the UI receives initial data
                        match &s.db_pool {
                            Some(p) => {
                                // First attempt to generate missing covers synchronously in the async runtime
                                match crate::commands::library::generate_missing_covers_startup(p, app_handle.clone()).await {
                                    Ok(n) => log::info!("Auto-generated {} missing covers at startup", n),
                                    Err(e) => log::warn!("generate_missing_covers_startup failed: {}", e),
                                }

                                match crate::library::sqlite_repository::MangaRepository::list_manga(p, 1, 1000).await {
                                    Ok((list, _)) => {
                                        if let Ok(items_json) = serde_json::to_value(list) {
                                            let _ = app_handle.emit_all("library-updated", serde_json::json!({"status":"ok","items": items_json}));
                                        }
                                    }
                                    Err(e) => log::warn!("Failed to query manga for initial emit: {}" , e),
                                }

                                // Emit downloads snapshot
                                let downloads = crate::commands::download::get_downloads_snapshot().await;
                                let _ = app_handle.emit_all("downloads-updated", downloads);
                            }
                            None => log::warn!("DB pool missing when attempting initial library emit"),
                        }
                        log::info!("[ASYNC-INIT] ✓ Async initialization completed successfully!");
                    });
                } else {
                    log::error!("[ASYNC-INIT] Failed to create tokio runtime");
                }
            });

            log::info!("[SETUP] ✓ Setup completed (background initialization started)!");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Library commands
            commands::library::get_library,
            commands::library::get_library_path_cmd,
            commands::library::set_library_path_cmd,
            
            // Search commands
            commands::search::search_manga,
            commands::search::list_manga_by_source,
            commands::search::search_web,
            commands::search::get_manga_details,
            
            // Download commands
            commands::download::start_download,
            commands::download::remove_download,
            commands::download::list_downloads,
            commands::download::get_download_progress,
            commands::download::list_downloaded_items,
            commands::download::remove_downloaded_manga,
            
            // Reader commands
            commands::reader::list_local_chapters,
            commands::reader::get_chapter_pages,
            commands::reader::mark_chapter_read,
            
            // Favorites commands (stubs)
            commands::favorites::add_to_favorites,
            commands::favorites::remove_from_favorites,
            commands::favorites::get_favorites,
            
            // Sources commands (stubs)
            commands::sources::get_available_sources,
            commands::sources::update_source_settings,
            
            // Settings commands (stubs)
            commands::settings::get_settings,
            commands::settings::update_settings,
            
            // Admin commands
            commands::admin::populate_test_data,
            // Force rescan
            commands::library::force_rescan_and_refresh,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure logs directory exists (use config helper if possible)
    let logs_dir = match config::init::get_logs_path() {
        Ok(p) => p,
        Err(_) => std::env::current_dir()?.join("logs"),
    };
    std::fs::create_dir_all(&logs_dir)?;

    let log_file_path = logs_dir.join("firescan.log");

    let file = fern::log_file(&log_file_path)?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(file)
        .apply()?;

    Ok(())
}
