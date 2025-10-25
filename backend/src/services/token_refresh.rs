use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use crate::services::OAuthRefreshService;

/// Background service to proactively refresh OAuth tokens before they expire
pub async fn start_token_refresh_service(pool: Arc<SqlitePool>) {
    log::info!("Starting OAuth token refresh service...");
    
    let mut interval = interval(Duration::from_secs(3000)); // Check every 50 minutes
    
    loop {
        interval.tick().await;
        
        log::info!("Running OAuth token refresh check...");
        
        // Get all OAuth users
        let users_result = sqlx::query_as::<_, (i64, String)>(
            "SELECT id, email FROM users WHERE oauth_provider IS NOT NULL"
        )
        .fetch_all(pool.as_ref())
        .await;
        
        match users_result {
            Ok(users) => {
                log::info!("Found {} OAuth users to check", users.len());
                
                for (user_id, email) in users {
                    log::info!("Refreshing token for user {} ({})", user_id, email);
                    
                    let refresh_service = OAuthRefreshService::new();
                    match refresh_service.refresh_token(pool.as_ref(), user_id).await {
                        Ok(_) => {
                            log::info!("✅ Token refreshed successfully for user {}", user_id);
                        }
                        Err(e) => {
                            log::warn!("Failed to refresh token for user {}: {}", user_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to fetch OAuth users: {}", e);
            }
        }
    }
}
