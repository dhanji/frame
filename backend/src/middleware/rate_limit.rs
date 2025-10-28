use actix_web::{dev::ServiceRequest, Error};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> bool {
        let mut requests = self.requests.write().await;
        let now = Instant::now();
        
        let user_requests = requests.entry(key.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests outside the window
        user_requests.retain(|&req_time| now.duration_since(req_time) < self.window);
        
        if user_requests.len() < self.max_requests {
            user_requests.push(now);
            true
        } else {
            false
        }
    }
}

pub async fn rate_limit_middleware(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, Error> {
    let rate_limiter = req.app_data::<RateLimiter>()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Rate limiter not configured"))?;
    
    let key = credentials.token();
    
    if !rate_limiter.check_rate_limit(key).await {
        return Err(actix_web::error::ErrorTooManyRequests("Rate limit exceeded"));
    }
    
    Ok(req)
}