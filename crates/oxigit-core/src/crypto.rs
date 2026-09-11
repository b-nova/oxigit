use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hkdf::Hkdf;
use hmac::Hmac;
use sha2::Sha256;

use crate::error::{OxigitError, Result};

const NONCE_LEN: usize = 12;
const ENC_PREFIX: &str = "enc:";

fn derive_encryption_key(master_key: &[u8]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(None, master_key);
    let mut key = [0u8; 32];
    hkdf.expand(b"oxigit-api-key-encryption", &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

/// Encrypt a plaintext string using AES-256-GCM.
/// Returns `"enc:<base64(nonce || ciphertext)>"`.
pub fn encrypt_secret(master_key: &[u8], plaintext: &str) -> Result<String> {
    let key = derive_encryption_key(master_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| OxigitError::Crypto(format!("cipher init: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| OxigitError::Crypto(format!("encryption failed: {e}")))?;

    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(format!("{}{}", ENC_PREFIX, BASE64.encode(&combined)))
}

/// Decrypt a stored value using AES-256-GCM.
/// If it lacks the `enc:` prefix, returns it as-is (legacy plaintext).
pub fn decrypt_secret(master_key: &[u8], stored: &str) -> Result<String> {
    if !stored.starts_with(ENC_PREFIX) {
        return Ok(stored.to_string());
    }

    let encoded = &stored[ENC_PREFIX.len()..];
    let combined = BASE64
        .decode(encoded)
        .map_err(|e| OxigitError::Crypto(format!("base64 decode: {e}")))?;

    if combined.len() < NONCE_LEN {
        return Err(OxigitError::Crypto("ciphertext too short".into()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let key = derive_encryption_key(master_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| OxigitError::Crypto(format!("cipher init: {e}")))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        OxigitError::Crypto("decryption failed (wrong key or tampered data)".into())
    })?;

    String::from_utf8(plaintext)
        .map_err(|e| OxigitError::Crypto(format!("decrypted data not UTF-8: {e}")))
}

/// Returns true if the value is already encrypted (has the `enc:` prefix).
pub fn is_encrypted(stored: &str) -> bool {
    stored.starts_with(ENC_PREFIX)
}

const HOOK_TOKEN_LABEL: &[u8] = b"oxigit-guardrail-hook";

/// Derive the token that git hooks present to `/internal/guardrail-check`.
/// This is a purpose-bound HMAC of the master key, so the master key itself
/// is never handed to hook processes.
pub fn hook_token(master_key: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(master_key).expect("HMAC-SHA256 accepts keys of any length");
    hmac::Mac::update(&mut mac, HOOK_TOKEN_LABEL);
    hex::encode(hmac::Mac::finalize(mac).into_bytes())
}

/// Constant-time check of a hook token presented by a caller.
pub fn verify_hook_token(master_key: &[u8], provided: &str) -> bool {
    let Ok(provided) = hex::decode(provided) else {
        return false;
    };
    let mut mac =
        Hmac::<Sha256>::new_from_slice(master_key).expect("HMAC-SHA256 accepts keys of any length");
    hmac::Mac::update(&mut mac, HOOK_TOKEN_LABEL);
    hmac::Mac::verify_slice(mac, &provided).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![0xAB; 32]
    }

    #[test]
    fn roundtrip() {
        let key = test_key();
        let plaintext = "sk-proj-abc123xyz";
        let encrypted = encrypt_secret(&key, plaintext).unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX));
        let decrypted = decrypt_secret(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn hook_token_verifies_and_differs_from_key() {
        let key = test_key();
        let token = hook_token(&key);
        assert_ne!(token, hex::encode(&key));
        assert!(verify_hook_token(&key, &token));
        assert!(!verify_hook_token(&key, &hex::encode(&key)));
        assert!(!verify_hook_token(&key, ""));
        assert!(!verify_hook_token(&key, "not-hex"));
        assert!(!verify_hook_token(&[0xCD; 32], &token));
    }

    #[test]
    fn different_ciphertext_each_time() {
        let key = test_key();
        let plaintext = "sk-proj-abc123xyz";
        let enc1 = encrypt_secret(&key, plaintext).unwrap();
        let enc2 = encrypt_secret(&key, plaintext).unwrap();
        assert_ne!(enc1, enc2);
        assert_eq!(decrypt_secret(&key, &enc1).unwrap(), plaintext);
        assert_eq!(decrypt_secret(&key, &enc2).unwrap(), plaintext);
    }

    #[test]
    fn plaintext_passthrough() {
        let key = test_key();
        let plaintext = "sk-proj-abc123xyz";
        let decrypted = decrypt_secret(&key, plaintext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn is_encrypted_check() {
        assert!(is_encrypted("enc:abc123"));
        assert!(!is_encrypted("sk-proj-abc123xyz"));
        assert!(!is_encrypted(""));
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = vec![0xAB; 32];
        let key2 = vec![0xCD; 32];
        let encrypted = encrypt_secret(&key1, "secret").unwrap();
        assert!(decrypt_secret(&key2, &encrypted).is_err());
    }

    #[test]
    fn empty_string_roundtrip() {
        let key = test_key();
        let encrypted = encrypt_secret(&key, "").unwrap();
        let decrypted = decrypt_secret(&key, &encrypted).unwrap();
        assert_eq!(decrypted, "");
    }
}
