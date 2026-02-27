use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::rngs::OsRng;
use rand::RngCore;
use std::env;

/// Retrieves the encryption key from the environment.
/// Must be a 32-byte hex string or exactly 32 bytes long string.
fn get_encryption_key() -> Key<Aes256Gcm> {
    let key_str = env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
        tracing::warn!("ENCRYPTION_KEY not set. Using a static fallback key for dev. Do not use in production!");
        "0123456789abcdef0123456789abcdef".to_string()
    });
    
    // If hex-encoded
    if key_str.len() == 64 {
        let decoded = hex::decode(&key_str).unwrap_or_else(|_| vec![0; 32]);
        *Key::<Aes256Gcm>::from_slice(&decoded)
    } else {
        // Assume UTF-8 up to 32 bytes
        let mut key_bytes = [0u8; 32];
        let bytes = key_str.as_bytes();
        let len = std::cmp::min(bytes.len(), 32);
        key_bytes[..len].copy_from_slice(&bytes[..len]);
        key_bytes.into()
    }
}

pub fn encrypt(plaintext: &str) -> Result<String, crate::errors::AppError> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }

    let key = get_encryption_key();
    let cipher = Aes256Gcm::new(&key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).map_err(|e| {
        crate::errors::AppError::Internal(format!("Encryption failed: {}", e))
    })?;

    // Combine nonce and ciphertext: nonce || ciphertext
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(combined))
}

pub fn decrypt(ciphertext_b64: &str) -> Result<String, crate::errors::AppError> {
    if ciphertext_b64.is_empty() {
        return Ok(String::new());
    }

    let combined = STANDARD.decode(ciphertext_b64).map_err(|e| {
        crate::errors::AppError::Internal(format!("Base64 decoding failed: {}", e))
    })?;

    if combined.len() < 12 {
        return Err(crate::errors::AppError::Internal("Ciphertext too short".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key = get_encryption_key();
    let cipher = Aes256Gcm::new(&key);

    let plaintext_bytes = cipher.decrypt(nonce, ciphertext).map_err(|e| {
        crate::errors::AppError::Internal(format!("Decryption failed: {}", e))
    })?;

    String::from_utf8(plaintext_bytes).map_err(|e| {
        crate::errors::AppError::Internal(format!("Invalid UTF-8 in plaintext: {}", e))
    })
}
