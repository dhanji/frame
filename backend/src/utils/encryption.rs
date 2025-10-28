use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce, Key,
};
use base64::{Engine as _, engine::general_purpose};
use std::env;

#[derive(Clone)]
pub struct Encryption {
    cipher: Aes256Gcm,
}

impl Encryption {
    pub fn new() -> Self {
        let key_string = env::var("ENCRYPTION_KEY")
            .unwrap_or_else(|_| "0123456789abcdef0123456789abcdef".to_string());
        
        // Try to decode as base64 first, fall back to raw bytes if that fails
        let key_bytes = general_purpose::STANDARD
            .decode(&key_string)
            .unwrap_or_else(|_| key_string.as_bytes().to_vec());
        
        // Ensure key is exactly 32 bytes
        let mut key_array = [0u8; 32];
        let len = key_bytes.len().min(32);
        key_array[..len].copy_from_slice(&key_bytes[..len]);
        
        let key = Key::<Aes256Gcm>::from_slice(&key_array);
        let cipher = Aes256Gcm::new(key);
        
        Self { cipher }
    }
    
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        // Generate a random nonce (12 bytes for AES-GCM)
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        // Prepend nonce to ciphertext for storage
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(general_purpose::STANDARD.encode(result))
    }
    
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, String> {
        let decoded = general_purpose::STANDARD
            .decode(ciphertext)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;
        
        if decoded.len() < 12 {
            return Err("Invalid ciphertext: too short".to_string());
        }
        
        // Extract nonce from the beginning of the decoded data
        let nonce = Nonce::from_slice(&decoded[0..12]);
        let ciphertext_bytes = &decoded[12..];
        
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|e| format!("Decryption failed: {}", e))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| format!("UTF-8 conversion failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption_decryption() {
        let encryption = Encryption::new();
        let plaintext = "test_password_123";
        
        let encrypted = encryption.encrypt(plaintext).unwrap();
        assert_ne!(encrypted, plaintext);
        
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}