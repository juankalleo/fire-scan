use serde_json::json;

#[tauri::command]
pub async fn get_settings() -> Result<serde_json::Value, String> {
    // TODO: Query user_settings table
    log::info!("Getting settings");
    
    Ok(json!({
        "theme": "dark",
        "language": "pt-BR",
        "concurrency": 4
    }))
}

#[tauri::command]
pub async fn update_settings(settings: serde_json::Value) -> Result<serde_json::Value, String> {
    // TODO: Update user_settings table
    log::info!("Updating settings: {:?}", settings);
    
    Ok(json!({
        "status": "success"
    }))
}
