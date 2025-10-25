use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, RefreshToken,
    TokenResponse, TokenUrl,
};
use oauth2::reqwest::async_http_client;
use sqlx::SqlitePool;
use crate::models::User;
use crate::utils::encryption::Encryption;

/// OAuth token refresh service
pub struct OAuthRefreshService {
    encryption: Encryption,
}

impl OAuthRefreshService {
    pub fn new() -> Self {
        Self {
            encryption: Encryption::new(),
        }
    }

    /// Get Google OAuth client
    fn get_google_oauth_client() -> Result<BasicClient, String> {
        let google_client_id = std::env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| "GOOGLE_CLIENT_ID not set".to_string())?;
        let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| "GOOGLE_CLIENT_SECRET not set".to_string())?;
        let redirect_url = std::env::var("GOOGLE_REDIRECT_URL")
            .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string());

        let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
            .map_err(|e| format!("Invalid authorization endpoint URL: {}", e))?;
        let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
            .map_err(|e| format!("Invalid token endpoint URL: {}", e))?;

        Ok(BasicClient::new(
            ClientId::new(google_client_id),
            Some(ClientSecret::new(google_client_secret)),
            auth_url,
            Some(token_url),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_url)
                .map_err(|e| format!("Invalid redirect URL: {}", e))?,
        ))
    }

    /// Refresh OAuth access token for a user
    pub async fn refresh_token(
        &self,
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Attempting to refresh OAuth token for user {}", user_id);

        // Get user from database
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

        // Check if user has OAuth provider
        if user.oauth_provider.is_none() {
            return Err("User is not an OAuth user".into());
        }

        // Get refresh token
        let encrypted_refresh_token = user
            .oauth_refresh_token
            .ok_or("No refresh token available")?;

        let refresh_token_str = self.encryption.decrypt(&encrypted_refresh_token)?;

        // Get OAuth client
        let client = Self::get_google_oauth_client()
            .map_err(|e| format!("OAuth client error: {}", e))?;

        // Exchange refresh token for new access token
        log::info!("Exchanging refresh token for new access token for user {}", user_id);
        let token_result = client
            .exchange_refresh_token(&RefreshToken::new(refresh_token_str))
            .request_async(async_http_client)
            .await
            .map_err(|e| {
                log::error!("Token refresh failed for user {}: {:?}", user_id, e);
                format!("Token refresh failed: {:?}", e)
            })?;

        let new_access_token = token_result.access_token().secret();

        // Encrypt and store new access token
        let encrypted_access_token = self.encryption.encrypt(new_access_token)?;

        sqlx::query(
            "UPDATE users SET oauth_access_token = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(&encrypted_access_token)
        .bind(user_id)
        .execute(pool)
        .await?;

        log::info!("✅ OAuth token refreshed successfully for user {}", user_id);

        Ok(new_access_token.to_string())
    }

    /// Get valid OAuth token for a user (refresh if needed)
    pub async fn get_valid_token(
        &self,
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Get user from database
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

        // Check if user has OAuth provider
        if user.oauth_provider.is_none() {
            return Err("User is not an OAuth user".into());
        }

        // Get current access token
        let encrypted_access_token = user
            .oauth_access_token
            .ok_or("No access token available")?;

        let access_token = self.encryption.decrypt(&encrypted_access_token)?;

        // Return current token - if it's expired, the caller will detect auth failure
        // and call refresh_token explicitly
        Ok(access_token)
    }
}

impl Default for OAuthRefreshService {
    fn default() -> Self {
        Self::new()
    }
}
