use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\app.db";
    let covers_dir = PathBuf::from("C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\covers");
    fs::create_dir_all(&covers_dir)?;

    println!("Opening DB: {}", db_path);
    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare("SELECT id, local_path, cover_path FROM manga")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)))?;

    for r in rows {
        let (id, local_path_opt, cover_opt) = r?;
        if let Some(c) = cover_opt {
            if !c.trim().is_empty() {
                println!("{} already has cover: {} (will still attempt CBZ extraction)", id, c);
                // do not continue: we will still try to extract from CBZs to force a canonical cover
            }
        }

        let local_path = match local_path_opt {
            Some(p) => p,
            None => continue,
        };

        println!("Processing {} -> {}", id, local_path);
        let p = Path::new(&local_path);
        let mut found: Option<PathBuf> = None;

        // Check common cover names
        for name in &["cover.jpg", "cover.png", "cover.webp"] {
            let cp = p.join(name);
            if cp.exists() {
                found = Some(cp);
                break;
            }
        }

        // Search for first image file recursively (deeper)
        if found.is_none() && p.exists() {
            for entry in WalkDir::new(p).max_depth(8).into_iter().filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        let ext = ext.to_lowercase();
                        if ["jpg","jpeg","png","webp"].contains(&ext.as_str()) {
                            found = Some(entry.path().to_path_buf());
                            break;
                        }
                    }
                }
            }
        }

        // If still none, search for CBZ/ZIP files recursively and extract first image found
        if found.is_none() && p.exists() {
            for entry in WalkDir::new(p).max_depth(6).into_iter().filter_map(|e| e.ok()) {
                let ep = entry.path();
                if !ep.is_file() { continue; }
                if let Some(ext) = ep.extension().and_then(|s| s.to_str()) {
                    let ext_l = ext.to_lowercase();
                    if ext_l == "cbz" || ext_l == "zip" {
                        if let Ok(file) = fs::File::open(&ep) {
                            if let Ok(mut archive) = ZipArchive::new(file) {
                                // iterate entries and pick the first image-like file
                                for i in 0..archive.len() {
                                    if let Ok(mut f) = archive.by_index(i) {
                                        if let Some(name) = Path::new(f.name()).extension().and_then(|s| s.to_str()) {
                                            let name_l = name.to_lowercase();
                                            if ["jpg","jpeg","png","webp"].contains(&name_l.as_str()) {
                                                let out_path = covers_dir.join(format!("{}.{}", id, name_l));
                                                let mut out = fs::File::create(&out_path)?;
                                                std::io::copy(&mut f, &mut out)?;
                                                found = Some(out_path);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if found.is_some() { break; }
            }
        }

        if let Some(fp) = found {
            let fp_s = fp.to_string_lossy().to_string();
            println!("Found cover for {} -> {}", id, fp_s);
            conn.execute("UPDATE manga SET cover_path = ?1 WHERE id = ?2", params![fp_s, id])?;
        } else {
            println!("No cover found for {}. Creating placeholder cover.", id);
            // create a simple placeholder image (solid color) and save as PNG
            let covers_dir = covers_dir.clone();
            let target = covers_dir.join(format!("{}.png", id));
            // derive a pseudo-random color from id
            let mut hash: u64 = 1469598103934665603u64;
            for b in id.as_bytes() {
                hash ^= *b as u64;
                hash = hash.wrapping_mul(1099511628211u64);
            }
            let r = ((hash >> 16) & 0xFF) as u8;
            let g = ((hash >> 8) & 0xFF) as u8;
            let b = (hash & 0xFF) as u8;

            let imgx = 600u32;
            let imgy = 900u32;
            let mut im = image::RgbImage::new(imgx, imgy);
            for (_x, _y, pixel) in im.enumerate_pixels_mut() {
                *pixel = image::Rgb([r, g, b]);
            }
            if let Err(e) = im.save(&target) {
                println!("Failed to save placeholder for {}: {}", id, e);
            } else {
                let tp = target.to_string_lossy().to_string();
                conn.execute("UPDATE manga SET cover_path = ?1 WHERE id = ?2", params![tp, id])?;
                println!("Placeholder cover created for {} -> {}", id, tp);
            }
        }
    }

    println!("Done filling covers");
    Ok(())
}
