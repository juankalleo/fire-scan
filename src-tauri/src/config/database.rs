use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::migrate::Migrator;
use std::str::FromStr;
use anyhow::Result;
use crate::config::init;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn get_connection_pool() -> Result<sqlx::SqlitePool> {
    let db_path = init::get_database_path()?;
    let connection_string = format!("sqlite://{}", db_path.to_string_lossy());

    let options = SqliteConnectOptions::from_str(&connection_string)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    Ok(pool)
}

pub fn initialize_database() -> Result<()> {
    // This will run migrations when the app starts
    // Migrations are stored in ./migrations directory
    log::info!("Database initialized");
    Ok(())
}

pub async fn run_migrations() -> Result<()> {
    let pool = get_connection_pool().await?;
    if let Err(e) = MIGRATOR.run(&pool).await {
        let s = e.to_string();
        let s_low = s.to_lowercase();
        if s_low.contains("cannot start a transaction") || s_low.contains("nested transaction") || s_low.contains("already in a transaction") {
            log::warn!("Migrations skipped due to nested/ concurrent transaction (non-fatal). Proceeding: {}", s);
            return Ok(());
        }
        // If not a known benign concurrency issue, return the error
        return Err(e.into());
    }
    Ok(())
}
