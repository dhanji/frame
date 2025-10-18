use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration: i64,
    pub server_host: String,
    pub server_port: u16,
    pub max_attachment_size: usize,
    pub cache_ttl: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "sqlite:email_client.db".to_string(),
            jwt_secret: "your-secret-key-change-in-production".to_string(),
            jwt_expiration: 86400, // 24 hours
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            max_attachment_size: 25 * 1024 * 1024, // 25MB
            cache_ttl: 300, // 5 minutes
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let mut config = Config::default();
        
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.database_url = url;
        }
        
        if let Ok(secret) = std::env::var("JWT_SECRET") {
            config.jwt_secret = secret;
        }
        
        if let Ok(exp) = std::env::var("JWT_EXPIRATION") {
            if let Ok(exp_val) = exp.parse() {
                config.jwt_expiration = exp_val;
            }
        }
        
        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server_host = host;
        }
        
        if let Ok(port) = std::env::var("SERVER_PORT") {
            if let Ok(port_val) = port.parse() {
                config.server_port = port_val;
            }
        }
        
        config
    }
}