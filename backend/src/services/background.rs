use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time;
use web::Data;

pub async fn start_background_tasks(pool: Data<SqlitePool>) {
    log::info!("Starting background tasks...");
    
    // Email sync task
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(300)); // Every 5 minutes
        
        loop {
            interval.tick().await;
            
            if let Err(e) = sync_all_user_emails(&pool_clone).await {
                log::error!("Error syncing emails: {}", e);
            }
        }
    });
    
    // Cleanup task
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(86400)); // Every 24 hours
        
        loop {
            interval.tick().await;
            
            if let Err(e) = cleanup_old_emails(&pool_clone).await {
                log::error!("Error cleaning up old emails: {}", e);
            }
        }
    });
    
    // Session cleanup task
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(3600)); // Every hour
        
        loop {
            interval.tick().await;
            
            if let Err(e) = cleanup_expired_sessions(&pool_clone).await {
                log::error!("Error cleaning up sessions: {}", e);
            }
        }
    });
}

async fn sync_all_user_emails(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Syncing emails for all users...");
    
    // Get all active users
    let users: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE is_active = true"
    )
    .fetch_all(pool)
    .await?;
    
    for user_id in users {
        // Create email service and sync
        let email_service = crate::services::EmailService::new(pool.clone());
        
        if let Err(e) = email_service.sync_emails(user_id).await {
            log::error!("Error syncing emails for user {}: {}", user_id, e);
        }
    }
    
    Ok(())
}

async fn cleanup_old_emails(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Cleaning up old deleted emails...");
    
    // Delete emails in trash older than 30 days
    sqlx::query(
        r#"
        DELETE FROM emails 
        WHERE folder = 'Trash' 
        AND deleted_at IS NOT NULL 
        AND deleted_at < datetime('now', '-30 days')
        "#
    )
    .execute(pool)
    .await?;
    
    // Delete old drafts (older than 90 days)
    sqlx::query(
        r#"
        DELETE FROM drafts 
        WHERE updated_at < datetime('now', '-90 days')
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn cleanup_expired_sessions(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Cleaning up expired sessions...");
    
    // Delete sessions older than 7 days
    sqlx::query(
        r#"
        DELETE FROM sessions 
        WHERE created_at < datetime('now', '-7 days')
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

use actix_web::web;