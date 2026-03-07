use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::env;

fn main() -> anyhow::Result<()> {
    let local = env::var("LOCALAPPDATA")?;
    let mut db = PathBuf::from(local);
    db.push("firescan");
    db.push("firescan");
    db.push("data");
    db.push("app.db");

    println!("DB path: {}", db.display());
    if !db.exists() {
        eprintln!("DB not found: {}", db.display());
        std::process::exit(1);
    }

    let conn = Connection::open(db)?;

    // Check if source_id exists
    let mut stmt = conn.prepare("PRAGMA table_info('manga')")?;
    let mut rows = stmt.query([])?;
    let mut has_source_id = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "source_id" {
            has_source_id = true;
            break;
        }
    }

    if has_source_id {
        println!("Column source_id already present. Nothing to do.");
        return Ok(());
    }

    println!("Adding column source_id to manga table...");
    conn.execute_batch(
        "BEGIN;
         ALTER TABLE manga ADD COLUMN source_id TEXT DEFAULT 'local';
         UPDATE manga SET source_id = 'local' WHERE source_id IS NULL OR source_id = '';
         CREATE INDEX IF NOT EXISTS idx_manga_source_id ON manga(source_id);
         COMMIT;",
    )?;

    println!("Migration applied: source_id added.");
    Ok(())
}
