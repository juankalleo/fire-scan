use rusqlite::params;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\app.db";
    println!("Opening DB: {}", db_path);
    let conn = rusqlite::Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, title, local_path, cover_path, source_id FROM manga ORDER BY title LIMIT 100",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut count = 0;
    for r in rows {
        let (id, title, local_path, cover_path, source_id) = r?;
        println!("- id={} title={} source_id={:?}", id, title, source_id);
        println!("  local_path={:?}", local_path);
        println!("  cover_path={:?}", cover_path);
        count += 1;
    }

    println!("Total rows printed: {}", count);
    Ok(())
}
