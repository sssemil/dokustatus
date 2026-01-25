use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::InfraError;
use crate::app_error::{AppError, AppResult};

const NONCE_LEN: usize = 12;

#[derive(Clone)]
pub struct ProcessCipher {
    key: aes_gcm::Key<Aes256Gcm>,
    hmac_key: Vec<u8>,
}

impl ProcessCipher {
    pub fn from_env() -> Result<Self, InfraError> {
        let key_b64 =
            std::env::var("PROCESS_NUMBER_KEY").map_err(|_| InfraError::ConfigMissing {
                var: "PROCESS_NUMBER_KEY",
            })?;
        Self::new_from_base64(&key_b64).map_err(InfraError::CipherInit)
    }

    pub fn new_from_base64(key_b64: &str) -> AppResult<Self> {
        let raw = general_purpose::STANDARD
            .decode(key_b64.as_bytes())
            .map_err(|e| AppError::Internal(format!("Invalid PROCESS_NUMBER_KEY: {e}")))?;
        if raw.len() != 32 {
            return Err(AppError::Internal(
                "PROCESS_NUMBER_KEY must decode to 32 bytes".into(),
            ));
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&raw);
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self {
            key: *key,
            hmac_key: raw,
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let cipher = Aes256Gcm::new(&self.key);
        let nonce_bytes = rand::random::<[u8; NONCE_LEN]>();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut buffer = Vec::with_capacity(NONCE_LEN + plaintext.len() + 16);
        buffer.extend_from_slice(nonce);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("encrypt failed: {e}")))?;
        buffer.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(buffer))
    }

    pub fn decrypt(&self, data_b64: &str) -> AppResult<String> {
        let data = general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .map_err(|e| AppError::Internal(format!("decrypt decode failed: {e}")))?;
        if data.len() <= NONCE_LEN {
            return Err(AppError::Internal("ciphertext too short".into()));
        }
        let (nonce_bytes, cipher_bytes) = data.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new(&self.key);
        let plaintext = cipher
            .decrypt(nonce, cipher_bytes)
            .map_err(|e| AppError::Internal(format!("decrypt failed: {e}")))?;
        String::from_utf8(plaintext).map_err(|e| AppError::Internal(e.to_string()))
    }

    pub fn hash(&self, value: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.hmac_key).expect("hmac key");
        mac.update(value.as_bytes());
        let result = mac.finalize().into_bytes();
        hex::encode(result)
    }
}
