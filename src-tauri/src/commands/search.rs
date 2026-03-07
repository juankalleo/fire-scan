use serde_json::json;
use crate::AppState;
use tauri::State;

async fn clone_db_pool(state: &State<'_, tokio::sync::Mutex<AppState>>) -> Option<sqlx::SqlitePool> {
    let guard = state.lock().await;
    guard.db_pool.clone()
}

#[tauri::command]
pub async fn search_manga(
    query: String,
    sources: Option<Vec<String>>,
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Searching for manga: {} with sources: {:?}", query, sources);
    
    if query.trim().is_empty() {
        return Ok(json!({
            "query": query,
            "results": [],
            "total": 0
        }));
    }

    let pool_opt = clone_db_pool(&state).await;
    if pool_opt.is_none() {
        return Ok(json!({
            "query": query,
            "results": [],
            "total": 0,
            "from_db": false
        }));
    }
    let pool = pool_opt.as_ref().unwrap();

    // Use tokio::time::timeout to prevent infinite hanging
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        crate::library::sqlite_repository::MangaRepository::search_by_title(pool, &query)
    ).await {
        Ok(Ok(manga_list)) => {
            let results = if let Some(source_ids) = sources {
                // Filter by sources if specified
                manga_list.into_iter()
                    .filter(|m| source_ids.contains(&m.source_id))
                    .collect::<Vec<_>>()
            } else {
                manga_list
            };
            
            let total = results.len();
            log::info!("Found {} results for query '{}'", total, query);
            
            Ok(json!({
                "query": query,
                "results": results,
                "total": total,
                "from_db": true
            }))
        }
        Ok(Err(e)) => {
            log::error!("Search error: {}", e);
            Err(format!("Search failed: {}", e))
        }
        Err(_) => {
            log::error!("Search timeout after 5 seconds");
            Err("Search timeout - no results found".to_string())
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_manga_by_source(
    source_id: String,
    page: Option<u32>,
    pageSize: Option<u32>,
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let sid = source_id;
    let page = page.unwrap_or(1).max(1);
    let page_size = pageSize.unwrap_or(20).max(1);

    log::info!("Listing manga for source: {} (page={} page_size={})", sid, page, page_size);

    let pool_opt = clone_db_pool(&state).await;
    let pool = pool_opt.as_ref();

    // Try to get paginated results from database (local cache) if DB is ready
    if let Some(pool_ref) = pool {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            crate::library::sqlite_repository::MangaRepository::list_by_source_paginated(pool_ref, &sid, page, page_size)
        ).await {
            Ok(Ok((manga_list, total_pages))) => {
                if !manga_list.is_empty() {
                    let total = manga_list.len();
                    log::info!("Found {} cached manga from source '{}', total_pages={}", total, sid, total_pages);

                    return Ok(json!({
                        "source_id": sid,
                        "results": manga_list,
                        "page": page,
                        "page_size": page_size,
                        "total_pages": total_pages,
                        "from_cache": true
                    }));
                }
                // fallthrough to web scraping if empty
            }
            Ok(Err(e)) => {
                log::warn!("Error querying local cache: {}", e);
            }
            Err(_) => {
                log::warn!("Timeout querying local cache");
            }
        }
    } else {
        log::info!("DB not ready; skipping cached query and using web scraping for source: {}", sid);
    }
    

    // If no cached results, try to scrape from web (lightweight: only titles + covers)
    log::info!("No cached results, scraping web (light) for source: {} (page={} page_size={})", sid, page, page_size);

    // MangaLivre has true server-side pagination. Use remote page directly instead of local slicing.
    if sid == "mangalivre" {
        match tokio::time::timeout(
            std::time::Duration::from_secs(16),
            crate::scraper::WebScraper::search_mangalivre_paginated("", page),
        )
        .await
        {
            Ok(Ok((mut page_items, total_pages))) => {
                if page_items.len() > page_size as usize {
                    page_items.truncate(page_size as usize);
                }

                // Do not block source listing on cache writes; this endpoint must return fast.

                return Ok(json!({
                    "source_id": sid,
                    "results": page_items,
                    "page": page,
                    "page_size": page_size,
                    "total_pages": total_pages.max(1),
                    "from_web": true
                }));
            }
            Ok(Err(e)) => {
                log::warn!("Error scraping MangaLivre page {}: {}", page, e);
                return Ok(json!({
                    "source_id": sid,
                    "results": [],
                    "page": page,
                    "page_size": page_size,
                    "total_pages": 1,
                    "error": "scrape_failed",
                    "message": "MangaLivre indisponivel no momento"
                }));
            }
            Err(_) => {
                log::error!("Timeout scraping MangaLivre page {}", page);
                return Ok(json!({
                    "source_id": sid,
                    "results": [],
                    "page": page,
                    "page_size": page_size,
                    "total_pages": 1,
                    "error": "scrape_timeout",
                    "message": "MangaLivre demorou para responder"
                }));
            }
        }
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::scraper::WebScraper::search_source_light(&sid, "")
    ).await {
        Ok(Ok(mut web_manga_list)) => {
            // perform simple pagination on the scraped list to avoid returning everything
            let total = web_manga_list.len() as u32;
            let total_pages = ((total + page_size - 1) / page_size).max(1);
            let start = ((page - 1) * page_size) as usize;
            let end = (start + page_size as usize).min(web_manga_list.len());
            let page_slice = if start < web_manga_list.len() { web_manga_list.drain(start..end).collect::<Vec<_>>() } else { vec![] };

            // Do not block source listing on cache writes; this endpoint must return fast.

            log::info!("Web scrape returned {} total, returning page {} ({} items)", total, page, page_slice.len());
            // If scraping returned nothing, provide an emergency fallback for Niadd
            if page_slice.is_empty() && sid == "niadd" {
                log::warn!("No results from Niadd scrape; returning emergency fallback samples");
                let mut samples = Vec::new();
                for i in 1..=6 {
                    samples.push(json!({
                        "id": format!("niadd_fallback_{}", i),
                        "title": format!("Exemplo Niadd {}", i),
                        "source_id": "niadd",
                        "source_name": "Niadd",
                        "cover_path": null,
                        "synopsis": "Exemplo gerado localmente",
                        "status": "ongoing",
                        "rating": 0.0,
                        "language": "Português",
                        "local_path": "",
                        "total_chapters": 0,
                        "downloaded_chapters": 0,
                        "last_updated": chrono::Local::now().to_rfc3339(),
                    }));
                }

                return Ok(json!({
                    "source_id": sid,
                    "results": samples,
                    "page": page,
                    "page_size": page_size,
                    "total_pages": 1,
                    "from_web": true,
                    "fallback": true,
                    "message": "Returned emergency fallback results for Niadd"
                }));
            }

            Ok(json!({
                "source_id": sid,
                "results": page_slice,
                "page": page,
                "page_size": page_size,
                "total_pages": total_pages,
                "from_web": true
            }))
        }
        Ok(Err(e)) => {
            log::warn!("Error scraping web: {}", e);
            // Return empty list on error - source might just be empty
            Ok(json!({
                "source_id": sid,
                "results": [],
                "page": page,
                "page_size": page_size,
                "total_pages": 1,
                "error": "scrape_failed"
            }))
        }
        Err(_) => {
            log::error!("Timeout scraping web after 25 seconds");
            // Return an empty result but include an error flag so UI can display a message
            Ok(json!({
                "source_id": sid,
                "results": [],
                "page": page,
                "page_size": page_size,
                "total_pages": 1,
                "error": "scrape_timeout",
                "message": "Request timed out while scraping the source"
            }))
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_manga_details(
    source_id: String,
    manga_id: String,
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let sid = source_id;

    log::info!("Getting manga details for {} in source {}", manga_id, sid);

    match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::scraper::WebScraper::get_manga_details(&sid, &manga_id)
    ).await {
        Ok(Ok(Some(manga))) => {
            // Optionally cache
            if let Some(pool_ref) = clone_db_pool(&state).await.as_ref() {
                if let Err(e) = crate::library::sqlite_repository::MangaRepository::upsert_manga(pool_ref, &manga).await {
                    log::warn!("Failed to cache manga details {}: {}", manga.id, e);
                }
            } else {
                log::warn!("DB pool not ready; skipping caching of manga details: {}", manga.id);
            }

            Ok(json!({ "manga": manga }))
        }
        Ok(Ok(None)) => Err("Manga not found on source".to_string()),
        Ok(Err(e)) => {
            log::error!("Error getting manga details: {}", e);
            Err(format!("Error: {}", e))
        }
        Err(_) => {
            log::error!("Timeout getting manga details after 15 seconds");
            Err("Request timeout - try again later".to_string())
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn search_web(
    source_id: String,
    query: Option<String>,
    page: Option<u32>,
    pageSize: Option<u32>,
    _state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let sid = source_id;
    let q = query.unwrap_or_default();
    let page = page.unwrap_or(1).max(1);
    let page_size = pageSize.unwrap_or(20).max(1);
    
    log::info!("=== SEARCH_WEB ===");
    log::info!("source_id: '{}', query: '{}'", sid, q);

    // Scrape web directly with longer timeout
    if sid == "mangalivre" {
        match tokio::time::timeout(
            std::time::Duration::from_secs(25),
            crate::scraper::WebScraper::search_mangalivre_paginated(&q, page),
        )
        .await
        {
            Ok(Ok((mut page_items, total_pages))) => {
                if page_items.len() > page_size as usize {
                    page_items.truncate(page_size as usize);
                }

                // Do not block search results on cache writes.

                return Ok(json!({
                    "source_id": sid,
                    "query": q,
                    "results": page_items,
                    "page": page,
                    "page_size": page_size,
                    "total_pages": total_pages.max(1),
                    "from_web": true
                }));
            }
            Ok(Err(e)) => {
                log::error!("Web scraper error: {}", e);
                return Err(format!("Scraper error: {}", e));
            }
            Err(_) => {
                log::error!("Web search timeout after 25 seconds");
                return Err("Search timeout - the website might be slow or unavailable".to_string());
            }
        }
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::scraper::WebScraper::search_source(&sid, &q)
    ).await {
        Ok(Ok(web_manga_list)) => {
            let total = web_manga_list.len();
            
            // Do not block search results on cache writes.
            
            log::info!("Web search found {} results for source '{}'", total, sid);
            
            Ok(json!({
                "source_id": sid,
                "query": q,
                "results": web_manga_list,
                "total": total,
                "from_web": true
            }))
        }
        Ok(Err(e)) => {
            log::error!("Web scraper error: {}", e);
            Err(format!("Scraper error: {}", e))
        }
        Err(_) => {
            log::error!("Web search timeout after 15 seconds");
            Err("Search timeout - the website might be slow or unavailable".to_string())
        }
    }
}
