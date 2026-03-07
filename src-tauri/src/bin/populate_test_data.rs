use rusqlite::params;
use chrono::Local;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Adjust this path if your app.db is located elsewhere
    let db_path = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\app.db";

    println!("Opening DB: {}", db_path);
    let mut conn = rusqlite::Connection::open(db_path)?;

    let tx = conn.transaction()?;

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
        let source_name = match source_id {
            "niadd" => "Niadd",
            "mangalivre" => "Manga Livre",
            "unionmangas" => "Union Mangás",
            "nexus" => "Nexus Scan",
            _ => "Local",
        };

        tx.execute(
            "INSERT OR IGNORE INTO manga (
                id, title, source_id, source_name, synopsis,
                status, rating, language, local_path,
                total_chapters, downloaded_chapters, last_updated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                title,
                source_id,
                source_name,
                format!("A popular manga series: {}", title),
                "ongoing",
                8.5f64,
                "Português",
                format!("{}\\{}", "C:\\Users\\combo\\Documents\\manwhas", title),
                chapters,
                0i64,
                Local::now().to_rfc3339(),
            ],
        )?;
    }

    tx.commit()?;
    println!("Inserted test manga (10)");
    Ok(())
}
