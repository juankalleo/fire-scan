use serde_json::json;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn populate_test_data(
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Populating test data...");

    let pool_opt = {
        let guard = state.lock().await;
        guard.db_pool.clone()
    };
    if pool_opt.is_none() {
        return Err("Database not ready".to_string());
    }
    let pool = pool_opt.as_ref().unwrap();
    
    // Sample manga data for testing
    let test_manga = vec![
        ("manga_001", "Bleach", "niadd", 366),
        ("manga_002", "Naruto", "niadd", 700),
        ("manga_003", "One Piece", "mangalivre", 1100),
        ("manga_004", "Death Note", "mangalivre", 37),
        ("manga_005", "Attack on Titan", "niadd", 139),
        ("manga_006", "My Hero Academia", "mangalivre", 426),
        ("manga_007", "Demon Slayer", "unionmangas", 205),
        ("manga_008", "Jujutsu Kaisen", "unionmangas", 271),
        ("manga_009", "Tokyo Ghoul", "nexus", 174),
        ("manga_010", "Chainsaw Man", "nexus", 97),
    ];
    
    for (id, title, source_id, chapters) in test_manga {
        let query = r#"
            INSERT OR IGNORE INTO manga (
                id, title, source_id, source_name, synopsis, 
                status, rating, language, local_path, 
                total_chapters, downloaded_chapters, last_updated
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        let source_name = match source_id {
            "niadd" => "Niadd",
            "mangalivre" => "Manga Livre",
            "unionmangas" => "Union Mangás",
            "nexus" => "Nexus Scan",
            _ => "Local",
        };
        
        sqlx::query(query)
            .bind(id)
            .bind(title)
            .bind(source_id)
            .bind(source_name)
            .bind(format!("A popular manga series: {}", title))
            .bind("ongoing")
            .bind(8.5)
            .bind("Português")
            .bind(format!("/library/{}", title))
            .bind(chapters)
            .bind(0) // downloaded_chapters
            .bind(chrono::Local::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to insert test data: {}", e))?;
    }
    
    log::info!("Test data populated successfully");
    
    Ok(json!({
        "status": "success",
        "message": "10 test manga inserted",
        "count": 10
    }))
}
