use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use timelord_common::error::AppError;

const NONCE_SIZE: usize = 12;

pub struct TokenEncryptor {
    cipher: Aes256Gcm,
}

impl TokenEncryptor {
    /// Create from a 32-byte hex-encoded key (64 hex chars).
    pub fn new(hex_key: &str) -> anyhow::Result<Self> {
        let key_bytes =
            hex::decode(hex_key).map_err(|e| anyhow::anyhow!("Invalid ENCRYPTION_KEY hex: {e}"))?;
        if key_bytes.len() != 32 {
            anyhow::bail!("ENCRYPTION_KEY must be exactly 32 bytes (64 hex chars)");
        }
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("AES key init failed: {e}"))?;
        Ok(Self { cipher })
    }

    /// Encrypt plaintext, returning (ciphertext, nonce).
    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::internal(format!("Encryption failed: {e}")))?;

        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    /// Decrypt ciphertext with the given nonce.
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String, AppError> {
        if nonce.len() != NONCE_SIZE {
            return Err(AppError::internal("Invalid nonce size"));
        }
        let nonce = Nonce::from_slice(nonce);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::internal(format!("Decryption failed: {e}")))?;
        String::from_utf8(plaintext).map_err(|e| AppError::internal(format!("UTF-8 decode: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = "0".repeat(64);
        let enc = TokenEncryptor::new(&key).unwrap();
        let plaintext = "my-secret-token";
        let (ct, nonce) = enc.encrypt(plaintext).unwrap();
        let recovered = enc.decrypt(&ct, &nonce).unwrap();
        assert_eq!(plaintext, recovered);
    }
}
