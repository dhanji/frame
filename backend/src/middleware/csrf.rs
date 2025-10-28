use actix_web::{dev::ServiceRequest, Error};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use actix_web::web;

pub struct CsrfProtection {
    tokens: Arc<RwLock<HashMap<String, String>>>,
}

impl CsrfProtection {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn generate_token(&self, session_id: &str) -> String {
        // Generate a random token
        let mut bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        use base64::{Engine as _, engine::general_purpose};
        let token = general_purpose::STANDARD.encode(bytes);
        
        let mut tokens = self.tokens.write().await;
        tokens.insert(session_id.to_string(), token.clone());
        token
    }

    pub async fn validate_token(&self, session_id: &str, token: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens.get(session_id).map_or(false, |t| t == token)
    }

    pub async fn cleanup_old_tokens(&self) {
        let mut tokens = self.tokens.write().await;
        if tokens.len() > 10000 {
            tokens.clear();
        }
    }
}

// Simplified CSRF check function that returns proper Result type
pub async fn check_csrf_token(
    req: ServiceRequest,
    csrf: web::Data<CsrfProtection>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // Skip CSRF check for GET requests and auth endpoints
    if req.method() == actix_web::http::Method::GET 
        || req.path().starts_with("/api/auth/")
        || req.path().starts_with("/health") {
        return Ok(req);
    }

    // Get session ID from authenticated user - clone the value to avoid borrow issues
    let session_id = {
        // For now, use a placeholder session ID
        Some("placeholder".to_string())
    };
    
    let session_id = match session_id {
        Some(id) => id,
        None => return Err((actix_web::error::ErrorUnauthorized("Unauthorized"), req)),
    };

    // Get CSRF token from header
    let csrf_token = req.headers()
        .get("X-CSRF-Token")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    if let Some(token) = csrf_token {
        if csrf.validate_token(&session_id, &token).await {
            Ok(req)
        } else {
            Err((actix_web::error::ErrorForbidden("Invalid CSRF token"), req))
        }
    } else {
        Err((actix_web::error::ErrorForbidden("CSRF token required"), req))
    }
}