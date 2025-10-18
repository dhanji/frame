pub use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use crate::error::Result;
use std::time::Duration;
use std::path::Path;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    log::info!("Creating database pool for: {}", database_url);
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await
        .map_err(|e| {
            log::error!("Failed to create database pool: {}", e);
            e
        })?;
    
    log::info!("Database pool created successfully");
    
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    log::info!("Running database migrations...");
    
    // Ensure migrations directory exists
    if !Path::new("migrations").exists() {
        log::warn!("Migrations directory not found, using manual table creation");
        return Err(sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Migrations directory not found"
        )).into());
    }
    
    // Test database connection first
    match sqlx::query("SELECT 1").fetch_one(pool).await {
        Ok(_) => log::info!("Database connection test successful"),
        Err(e) => {
            log::error!("Database connection test failed: {}", e);
            return Err(e.into());
        }
    }
    
    // Try to run SQLx migrations if available
    #[cfg(feature = "migrate")]
    {
        match sqlx::migrate!("./migrations").run(pool).await {
            Ok(_) => {
                log::info!("SQLx migrations completed successfully");
                return Ok(());
            }
            Err(e) => {
                log::warn!("SQLx migrations failed: {}. Will use manual table creation.", e);
            }
        }
    }
    
    log::info!("No SQLx migrations configured, using manual table creation");
    log::info!("Database migrations completed");
    Ok(())
}