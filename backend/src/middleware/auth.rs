use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub user_id: i64,
    pub email: String,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: i64,  // Added for compatibility
    pub user_id: i64,
    pub username: String,
    pub email: String,
}

/// Demo mode user for testing without authentication
pub fn get_demo_user() -> AuthenticatedUser {
    AuthenticatedUser {
        id: 1,
        user_id: 1,
        username: "demo".to_string(),
        email: "demo@example.com".to_string(),
    }
}

/// Public function to validate token (used by WebSocket handler)
pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    
    Ok(token_data.claims)
}
/// Check if demo mode is enabled
pub fn is_demo_mode() -> bool {
    std::env::var("DEMO_MODE").unwrap_or_else(|_| "false".to_string()) == "true"
}

pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // Check if demo mode is enabled
    if is_demo_mode() {
        // In demo mode, inject demo user without authentication
        req.extensions_mut().insert(get_demo_user());
        return Ok(req);
    }
    
    // Check if this is a public endpoint that doesn't require auth
    let path = req.path();
    if path == "/health" || path == "/api/login" || path == "/api/register" {
        return Ok(req);
    }
    
    // Normal authentication flow
    {
        let token = credentials.token();
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        
        match decode_token(token, &secret) {
            Ok(user) => {
                req.extensions_mut().insert(user);
                Ok(req)
            }
            Err(_) => Err((
                actix_web::error::ErrorUnauthorized("Invalid token"),
                req
            )),
        }
    }
}

fn decode_token(token: &str, secret: &str) -> Result<AuthenticatedUser, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    
    Ok(AuthenticatedUser {
        id: token_data.claims.user_id,
        user_id: token_data.claims.user_id,
        username: token_data.claims.sub,
        email: token_data.claims.email,
    })
}

// Extractor for authenticated user
impl actix_web::FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        // Check demo mode first
        if is_demo_mode() {
            return ready(Ok(get_demo_user()));
        }
        
        // Try to get authenticated user from extensions
        if let Some(user) = req.extensions().get::<AuthenticatedUser>() {
            ready(Ok(user.clone()))
        } else {
            ready(Err(actix_web::error::ErrorUnauthorized("Not authenticated")))
        }
    }
}
