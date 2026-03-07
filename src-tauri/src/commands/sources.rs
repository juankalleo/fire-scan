use serde_json::json;
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub url: String,
    pub language: String,
    pub region: String,
    pub enabled: bool,
    pub priority: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

static SOURCES: OnceLock<Vec<Source>> = OnceLock::new();

pub fn load_sources() -> Vec<Source> {
    SOURCES.get_or_init(|| {
        let sources_json = include_str!("../sources.json");
        match serde_json::from_str::<Vec<Source>>(sources_json) {
            Ok(sources) => sources,
            Err(e) => {
                log::error!("Failed to parse sources.json: {}", e);
                default_sources()
            }
        }
    }).clone()
}

fn default_sources() -> Vec<Source> {
    vec![
        Source {
            id: "mangalivre".to_string(),
            name: "Manga Livre".to_string(),
            url: "https://mangalivre.to".to_string(),
            language: "Português".to_string(),
            region: "BR".to_string(),
            enabled: true,
            priority: 1,
            description: Some("Acervo enorme de mangá e manhwa".to_string()),
        },
        Source {
            id: "mangadex".to_string(),
            name: "MangaDex".to_string(),
            url: "https://mangadex.org".to_string(),
            language: "Multilíngue".to_string(),
            region: "GLOBAL".to_string(),
            enabled: true,
            priority: 2,
            description: Some("Plataforma global com várias traduções".to_string()),
        },
    ]
}

#[tauri::command]
pub async fn get_available_sources() -> Result<serde_json::Value, String> {
    log::info!("Getting available sources");

    // Temporary product decision: expose only Niadd and Manga Livre.
    let sources: Vec<Source> = load_sources()
        .into_iter()
        .filter(|s| s.id == "niadd" || s.id == "mangalivre")
        .collect();
    let enabled_count = sources.iter().filter(|s| s.enabled).count();
    
    Ok(json!({
        "sources": sources,
        "total": sources.len(),
        "enabled": enabled_count,
    }))
}

#[tauri::command]
pub async fn update_source_settings(
    source_id: String,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    log::info!("Updating source {} - enabled: {}", source_id, enabled);
    
    // In a real app, this would persist to database
    // For now, just acknowledge the change
    
    Ok(json!({
        "status": "success",
        "source_id": source_id,
        "enabled": enabled,
    }))
}

