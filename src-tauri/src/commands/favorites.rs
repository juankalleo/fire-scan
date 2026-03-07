use serde_json::json;
use sqlx::Row;
use tauri::State;
use uuid::Uuid;

use crate::AppState;

async fn clone_db_pool(state: &State<'_, tokio::sync::Mutex<AppState>>) -> Option<sqlx::SqlitePool> {
    let guard = state.lock().await;
    guard.db_pool.clone()
}

#[tauri::command]
pub async fn add_to_favorites(
    manga_id: String,
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Adding to favorites: {}", manga_id);

    let pool = clone_db_pool(&state)
        .await
        .ok_or_else(|| "DB pool not ready".to_string())?;

    let favorite_id = format!("fav_{}", Uuid::new_v4());

    match sqlx::query(
        "INSERT OR IGNORE INTO favorites (id, manga_id) VALUES (?, ?)",
    )
    .bind(&favorite_id)
    .bind(&manga_id)
    .execute(&pool)
    .await
    {
        Ok(_) => Ok(json!({
            "status": "success",
            "manga_id": manga_id
        })),
        Err(e) => Err(format!("Failed to add favorite: {}", e)),
    }
}

#[tauri::command]
pub async fn remove_from_favorites(
    manga_id: String,
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Removing from favorites: {}", manga_id);

    let pool = clone_db_pool(&state)
        .await
        .ok_or_else(|| "DB pool not ready".to_string())?;

    match sqlx::query("DELETE FROM favorites WHERE manga_id = ?")
        .bind(&manga_id)
        .execute(&pool)
        .await
    {
        Ok(_) => Ok(json!({
            "status": "success",
            "manga_id": manga_id
        })),
        Err(e) => Err(format!("Failed to remove favorite: {}", e)),
    }
}

#[tauri::command]
pub async fn get_favorites(
    state: State<'_, tokio::sync::Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    log::info!("Getting favorites");

    let pool = clone_db_pool(&state)
        .await
        .ok_or_else(|| "DB pool not ready".to_string())?;

    let rows = sqlx::query(
        "SELECT
            m.id,
            m.title,
            m.source_id,
            m.source_name,
            m.cover_path,
            m.synopsis,
            m.status,
            m.rating,
            m.language,
            m.local_path,
            m.total_chapters,
            m.downloaded_chapters,
            m.last_updated,
            f.added_at
         FROM favorites f
         JOIN manga m ON m.id = f.manga_id
         ORDER BY f.added_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to list favorites: {}", e))?;

    let favorites: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "title": r.get::<String, _>("title"),
                "source_id": r.get::<String, _>("source_id"),
                "source_name": r.get::<String, _>("source_name"),
                "cover_path": r.get::<Option<String>, _>("cover_path"),
                "synopsis": r.get::<Option<String>, _>("synopsis"),
                "status": r.get::<String, _>("status"),
                "rating": r.get::<f64, _>("rating"),
                "language": r.get::<String, _>("language"),
                "local_path": r.get::<String, _>("local_path"),
                "total_chapters": r.get::<i32, _>("total_chapters"),
                "downloaded_chapters": r.get::<i32, _>("downloaded_chapters"),
                "last_updated": r.get::<String, _>("last_updated"),
                "added_at": r.get::<Option<String>, _>("added_at"),
            })
        })
        .collect();

    Ok(json!({ "favorites": favorites }))
}
