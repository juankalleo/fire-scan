use sqlx::{SqlitePool, Row};
use anyhow::Result;
use super::library_service::Manga;
use log::info;

pub struct MangaRepository;

impl MangaRepository {
    /// Insert or update manga in database
    pub async fn upsert_manga(pool: &SqlitePool, manga: &Manga) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO manga (
                id, title, source_name, cover_path, synopsis,
                status, rating, language, local_path, total_chapters,
                downloaded_chapters, last_updated, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(&manga.id)
        .bind(&manga.title)
        .bind(&manga.source_name)
        .bind(&manga.cover_path)
        .bind(&manga.synopsis)
        .bind(&manga.status)
        .bind(manga.rating)
        .bind(&manga.language)
        .bind(&manga.local_path)
        .bind(manga.total_chapters)
        .bind(manga.downloaded_chapters)
        .bind(&manga.last_updated)
        .execute(pool)
        .await?;

        info!("Manga inserted/updated: {}", manga.title);
        Ok(())
    }

    /// Get manga by ID
    pub async fn get_manga_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Manga>> {
        let row = sqlx::query(
            "SELECT id, title, source_id, source_name, cover_path, synopsis,
                    status, rating, language, local_path, total_chapters,
                    downloaded_chapters, last_updated
             FROM manga WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| Manga {
            id: r.get("id"),
            title: r.get("title"),
            source_id: r.get("source_id"),
            source_name: r.get("source_name"),
            cover_path: r.get("cover_path"),
            synopsis: r.get("synopsis"),
            status: r.get("status"),
            rating: r.get("rating"),
            language: r.get("language"),
            local_path: r.get("local_path"),
            total_chapters: r.get("total_chapters"),
            downloaded_chapters: r.get("downloaded_chapters"),
            last_updated: r.get("last_updated"),
        }))
    }

    /// List all manga (paginated)
    pub async fn list_manga(
        pool: &SqlitePool,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<Manga>, u32)> {
        let offset = (page - 1) * page_size;

        // Get total count
        let count_row = sqlx::query(
            "SELECT COUNT(*) as count
             FROM manga
             WHERE TRIM(COALESCE(local_path, '')) <> ''"
        )
            .fetch_one(pool)
            .await?;
        let total: i32 = count_row.get("count");
        let total_pages = ((total as u32 + page_size - 1) / page_size).max(1);

        // Get manga list
        let rows = sqlx::query(
            "SELECT id, title, source_id, source_name, cover_path, synopsis,
                    status, rating, language, local_path, total_chapters,
                    downloaded_chapters, last_updated
               FROM manga
               WHERE TRIM(COALESCE(local_path, '')) <> ''
             ORDER BY last_updated DESC
             LIMIT ? OFFSET ?"
        )
        .bind(page_size as i32)
        .bind(offset as i32)
        .fetch_all(pool)
        .await?;

        let manga_list = rows
            .iter()
            .map(|r| Manga {
                id: r.get("id"),
                title: r.get("title"),
                source_id: r.get("source_id"),
                source_name: r.get("source_name"),
                cover_path: r.get("cover_path"),
                synopsis: r.get("synopsis"),
                status: r.get("status"),
                rating: r.get("rating"),
                language: r.get("language"),
                local_path: r.get("local_path"),
                total_chapters: r.get("total_chapters"),
                downloaded_chapters: r.get("downloaded_chapters"),
                last_updated: r.get("last_updated"),
            })
            .collect();

        Ok((manga_list, total_pages))
    }

    /// Search manga by title
    pub async fn search_by_title(pool: &SqlitePool, query: &str) -> Result<Vec<Manga>> {
        let search_pattern = format!("%{}%", query);
        
        let rows = sqlx::query(
            "SELECT id, title, source_id, source_name, cover_path, synopsis,
                    status, rating, language, local_path, total_chapters,
                    downloaded_chapters, last_updated
             FROM manga
             WHERE title LIKE ?
             ORDER BY title
             LIMIT 50"
        )
        .bind(&search_pattern)
        .fetch_all(pool)
        .await?;

        let manga_list = rows
            .iter()
            .map(|r| Manga {
                id: r.get("id"),
                title: r.get("title"),
                source_id: r.get("source_id"),
                source_name: r.get("source_name"),
                cover_path: r.get("cover_path"),
                synopsis: r.get("synopsis"),
                status: r.get("status"),
                rating: r.get("rating"),
                language: r.get("language"),
                local_path: r.get("local_path"),
                total_chapters: r.get("total_chapters"),
                downloaded_chapters: r.get("downloaded_chapters"),
                last_updated: r.get("last_updated"),
            })
            .collect();

        Ok(manga_list)
    }

    /// List manga by specific source
    pub async fn list_by_source(pool: &SqlitePool, source_id: &str) -> Result<Vec<Manga>> {
        // Backwards-compatible: return full list (used rarely)
        let rows = sqlx::query(
            "SELECT id, title, source_id, source_name, cover_path, synopsis,
                    status, rating, language, local_path, total_chapters,
                    downloaded_chapters, last_updated
             FROM manga
             WHERE source_id = ?
             ORDER BY title"
        )
        .bind(source_id)
        .fetch_all(pool)
        .await?;

        let manga_list = rows
            .iter()
            .map(|r| Manga {
                id: r.get("id"),
                title: r.get("title"),
                source_id: r.get("source_id"),
                source_name: r.get("source_name"),
                cover_path: r.get("cover_path"),
                synopsis: r.get("synopsis"),
                status: r.get("status"),
                rating: r.get("rating"),
                language: r.get("language"),
                local_path: r.get("local_path"),
                total_chapters: r.get("total_chapters"),
                downloaded_chapters: r.get("downloaded_chapters"),
                last_updated: r.get("last_updated"),
            })
            .collect();

        Ok(manga_list)
    }

    /// List manga by specific source (paginated)
    pub async fn list_by_source_paginated(
        pool: &SqlitePool,
        source_id: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<Manga>, u32)> {
        let offset = (page.saturating_sub(1)) * page_size;

        // Get total count for this source
        let count_row = sqlx::query("SELECT COUNT(*) as count FROM manga WHERE source_id = ?")
            .bind(source_id)
            .fetch_one(pool)
            .await?;
        let total: i32 = count_row.get("count");
        let total_pages = ((total as u32 + page_size - 1) / page_size).max(1);

        let rows = sqlx::query(
            "SELECT id, title, source_id, source_name, cover_path, synopsis,
                    status, rating, language, local_path, total_chapters,
                    downloaded_chapters, last_updated
             FROM manga
             WHERE source_id = ?
             ORDER BY title
             LIMIT ? OFFSET ?"
        )
        .bind(source_id)
        .bind(page_size as i32)
        .bind(offset as i32)
        .fetch_all(pool)
        .await?;

        let manga_list = rows
            .iter()
            .map(|r| Manga {
                id: r.get("id"),
                title: r.get("title"),
                source_id: r.get("source_id"),
                source_name: r.get("source_name"),
                cover_path: r.get("cover_path"),
                synopsis: r.get("synopsis"),
                status: r.get("status"),
                rating: r.get("rating"),
                language: r.get("language"),
                local_path: r.get("local_path"),
                total_chapters: r.get("total_chapters"),
                downloaded_chapters: r.get("downloaded_chapters"),
                last_updated: r.get("last_updated"),
            })
            .collect();

        Ok((manga_list, total_pages))
    }

    /// Get total count of manga
    pub async fn get_total_count(pool: &SqlitePool) -> Result<u32> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count
             FROM manga
             WHERE TRIM(COALESCE(local_path, '')) <> ''"
        )
            .fetch_one(pool)
            .await?;
        let count: i32 = row.get("count");
        Ok(count as u32)
    }

    /// Delete manga
    pub async fn delete_manga(pool: &SqlitePool, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM manga WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
