use crate::models::User;
use crate::services::{ImapService, SmtpService};
use crate::utils::encryption::Encryption;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Manages IMAP and SMTP services for all users
pub struct EmailManager {
    imap_services: Arc<RwLock<HashMap<i64, Arc<ImapService>>>>,
    smtp_services: Arc<RwLock<HashMap<i64, Arc<SmtpService>>>>,
    encryption: Encryption,
}

impl EmailManager {
    pub fn new() -> Self {
        Self {
            imap_services: Arc::new(RwLock::new(HashMap::new())),
            smtp_services: Arc::new(RwLock::new(HashMap::new())),
            encryption: Encryption::new(),
        }
    }

    /// Initialize email services for a user
    pub async fn initialize_user_blocking(
        &self,
        user: &User,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Decrypt email password
        let email_password = user
            .email_password
            .as_ref()
            .and_then(|p| self.encryption.decrypt(p).ok())
            .ok_or("Failed to decrypt email password")?;

        // Create and connect IMAP service
        log::info!("Initializing IMAP service for user {}", user.id);
        let imap_service = ImapService::new(
            user.imap_host.clone(),
            user.imap_port as u16,
            user.email.clone(),
            email_password.clone(),
        );
        
        // Try to connect to verify credentials
        if let Err(e) = imap_service.connect().await {
            log::warn!("Failed to connect to IMAP for user {}: {}", user.id, e);
            // Don't fail initialization, just log the warning
        } else {
            log::info!("IMAP connection successful for user {}", user.id);
        }
        
        // Store IMAP service
        self.imap_services
            .write()
            .await
            .insert(user.id, Arc::new(imap_service));

        // Create SMTP service
        log::info!("Initializing SMTP service for user {}", user.id);
        let smtp_service = SmtpService::new(
            user.smtp_host.clone(),
            user.smtp_port as u16,
            user.email.clone(),
            email_password,
            user.smtp_use_tls,
        );
        
        // Test SMTP connection
        if let Err(e) = smtp_service.test_connection().await {
            log::warn!("Failed to connect to SMTP for user {}: {}", user.id, e);
            // Don't fail initialization, just log the warning
        } else {
            log::info!("SMTP connection successful for user {}", user.id);
        }
        
        // Store SMTP service
        self.smtp_services
            .write()
            .await
            .insert(user.id, Arc::new(smtp_service));

        Ok(())
    }

    /// Initialize email services for a user (non-blocking with timeout)
    pub async fn initialize_user(
        &self,
        user: &User,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_clone = user.clone();
        let manager_clone = Self {
            imap_services: self.imap_services.clone(),
            smtp_services: self.smtp_services.clone(),
            encryption: self.encryption.clone(),
        };

        // Spawn initialization in background with timeout
        tokio::spawn(async move {
            let timeout_duration = Duration::from_secs(5);
            match tokio::time::timeout(
                timeout_duration,
                manager_clone.initialize_user_blocking(&user_clone)
            ).await {
                Ok(Ok(())) => {
                    log::info!("Email services initialized successfully for user {}", user_clone.id);
                }
                Ok(Err(e)) => {
                    log::warn!("Failed to initialize email services for user {}: {}", user_clone.id, e);
                }
                Err(_) => {
                    log::warn!("Email service initialization timed out for user {}", user_clone.id);
                }
            }
        });
        Ok(())
    }

    /// Get IMAP service for a user
    pub async fn get_imap_service(
        &self,
        user_id: i64,
    ) -> Option<Arc<ImapService>> {
        self.imap_services.read().await.get(&user_id).cloned()
    }

    /// Get SMTP service for a user
    pub async fn get_smtp_service(
        &self,
        user_id: i64,
    ) -> Option<Arc<SmtpService>> {
        self.smtp_services.read().await.get(&user_id).cloned()
    }

    /// Remove services for a user (on logout)
    pub async fn remove_user(&self, user_id: i64) {
        self.imap_services.write().await.remove(&user_id);
        self.smtp_services.write().await.remove(&user_id);
    }

    /// Check if user services are initialized
    pub async fn is_user_initialized(&self, user_id: i64) -> bool {
        self.imap_services.read().await.contains_key(&user_id)
    }
}

impl Default for EmailManager {
    fn default() -> Self {
        Self::new()
    }
}
