use rusqlite::params;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "C:\\Users\\combo\\AppData\\Local\\firescan\\firescan\\data\\app.db";
    println!("Opening DB: {}", db_path);
    let mut conn = rusqlite::Connection::open(db_path)?;

    // Delete any manga rows that are not local/manual (keep downloaded entries only)
    let tx = conn.transaction()?;
    let affected = tx.execute(
        "DELETE FROM manga WHERE source_id NOT IN ('local','manual')",
        params![],
    )?;
    tx.commit()?;

    println!("Deleted {} rows (non-local/manual).", affected);
    Ok(())
}
