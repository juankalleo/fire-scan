use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use walkdir::WalkDir;
use anyhow::Result;
use log::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manga {
    pub id: String,
    pub title: String,
    pub source_id: String,
    pub source_name: String,
    #[serde(alias = "coverImageUrl")]
    pub cover_path: Option<String>,
    pub synopsis: Option<String>,
    pub status: String,
    pub rating: f32,
    pub language: String,
    pub local_path: String,
    pub total_chapters: i32,
    pub downloaded_chapters: i32,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub manga_id: String,
    pub chapter_number: f32,
    pub title: Option<String>,
    pub downloaded: bool,
    pub file_path: Option<String>,
    pub pages: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaIndex {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub rating: f32,
    pub state: String,
    pub chapters: std::collections::HashMap<String, ChapterMeta>,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterMeta {
    pub number: f32,
    pub volume: Option<i32>,
    pub date: String,
    pub scanlator: String,
    pub filename: String,
}

pub struct LibraryService;

impl LibraryService {
    /// Escaneia diretório de biblioteca e retorna lista de mangás
    pub async fn scan_library(library_path: &Path) -> Result<Vec<Manga>> {
        info!("Scanning library at: {:?}", library_path);
        let mut manga_list = Vec::new();

        if !library_path.exists() {
            info!("Library path does not exist, creating it");
            fs::create_dir_all(library_path).await?;
            return Ok(Vec::new());
        }

        for entry in WalkDir::new(library_path)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "index.json" {
                if let Ok(manga) = Self::parse_manga_from_index(entry.path()).await {
                    manga_list.push(manga);
                }
            }
        }

        info!("Found {} manga titles", manga_list.len());
        Ok(manga_list)
    }

    /// Parse index.json and create Manga struct
    pub async fn parse_manga_from_index(index_path: &Path) -> Result<Manga> {
        let content = fs::read_to_string(index_path).await?;
        let meta: MangaIndex = serde_json::from_str(&content)?;

        let local_path = index_path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        let cover_path = Self::find_cover_image(&local_path).await.ok();

        let total_chapters = meta.chapters.len() as i32;
        let downloaded_chapters = total_chapters; // Wszystko w .cbz jest już pobrany

        Ok(Manga {
            id: meta.id.clone(),
            title: meta.title.clone(),
            source_id: String::from("local"),
            source_name: String::from("Biblioteca Local"),
            cover_path,
            synopsis: None,
            status: meta.state.clone(),
            rating: meta.rating,
            language: String::from("pt-BR"),
            local_path,
            total_chapters,
            downloaded_chapters,
            last_updated: chrono::Local::now().to_rfc3339(),
        })
    }

    /// Find cover image in manga folder
    async fn find_cover_image(manga_path: &str) -> Result<String> {
        const COVER_NAMES: &[&str] = &["cover.jpg", "cover.png", "cover.webp"];

        for name in COVER_NAMES {
            let path = Path::new(manga_path).join(name);
            if path.exists() {
                return Ok(path.to_string_lossy().to_string());
            }
        }

        Err(anyhow::anyhow!("No cover found"))
    }

    /// List chapters in a manga
    pub async fn get_chapters(manga_id: &str, local_path: &str) -> Result<Vec<Chapter>> {
        let index_path = Path::new(local_path).join("index.json");
        let content = fs::read_to_string(index_path).await?;
        let meta: MangaIndex = serde_json::from_str(&content)?;

        let chapters = meta
            .chapters
            .iter()
            .map(|(id, ch)| Chapter {
                id: id.clone(),
                manga_id: manga_id.to_string(),
                chapter_number: ch.number,
                title: Some(format!("Capítulo {}", ch.number)),
                downloaded: true,
                file_path: Some(format!("{}/{}", local_path, ch.filename)),
                pages: 0,
            })
            .collect();

        Ok(chapters)
    }
}
