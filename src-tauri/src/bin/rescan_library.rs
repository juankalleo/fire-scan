use rusqlite::params;
use std::fs;
use std::path::Path;
use chrono::Local;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\app.db";
    let lib_path_file = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\library_path.txt";

    println!("DB: {}", db_path);
    println!("Reading library path from: {}", lib_path_file);

    let lib_path = fs::read_to_string(lib_path_file)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("Failed to read library_path.txt: {}", e))?;

    if lib_path.is_empty() {
        return Err("library_path.txt is empty".into());
    }

    let mut conn = rusqlite::Connection::open(db_path)?;
    let tx = conn.transaction()?;

    let entries = fs::read_dir(&lib_path)
        .map_err(|e| format!("Failed to read library dir {}: {}", lib_path, e))?;

    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let id = format!("local_{}", title.replace(' ', "_").to_lowercase());

        // find cover: cover.jpg/png/webp or first image file
        let mut cover_path: Option<String> = None;
        let cover_candidates = ["cover.jpg", "cover.png", "cover.webp"];
        for c in &cover_candidates {
            let p = path.join(c);
            if p.exists() {
                cover_path = Some(p.to_string_lossy().to_string());
                break;
            }
        }
        if cover_path.is_none() {
            // search for first image
            for f in fs::read_dir(&path)? {
                let f = f?;
                let fpath = f.path();
                if let Some(ext) = fpath.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "webp"].contains(&ext.as_str()) {
                        cover_path = Some(fpath.to_string_lossy().to_string());
                        break;
                    }
                }
            }
        }

        let cover_path_db = cover_path.clone().unwrap_or_default();

        tx.execute(
            "INSERT OR REPLACE INTO manga (
                id, title, source_id, source_name, synopsis,
                status, rating, language, local_path, cover_path,
                total_chapters, downloaded_chapters, last_updated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                title,
                "local",
                "Local",
                format!("Imported from folder: {}", path.to_string_lossy()),
                "unknown",
                0f64,
                "Português",
                path.to_string_lossy().to_string(),
                cover_path_db,
                0i64,
                0i64,
                Local::now().to_rfc3339(),
            ],
        )?;
        count += 1;
    }

    tx.commit()?;
    println!("Rescan complete, inserted/updated {} manga entries", count);
    Ok(())
}
