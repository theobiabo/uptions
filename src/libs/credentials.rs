use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand_core::{OsRng, RngCore};
use serde_json::Value;

use crate::error::AppError;

pub fn parse_encryption_key(value: &str) -> Result<[u8; 32], AppError> {
    let trimmed = value.trim();
    let decoded = if trimmed.len() == 64
        && trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        decode_hex(trimmed)
            .map_err(|_| AppError::BadRequest("invalid encryption key".to_owned()))?
    } else {
        STANDARD
            .decode(trimmed)
            .unwrap_or_else(|_| trimmed.as_bytes().to_vec())
    };

    if decoded.len() != 32 {
        return Err(AppError::BadRequest(
            "credential encryption key must be 32 bytes".to_owned(),
        ));
    }

    let mut key = [0_u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

pub fn encrypt_json(key: &[u8; 32], value: &Value) -> Result<Value, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::DatabaseError("invalid encryption key".to_owned()))?;
    let mut nonce_bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext =
        serde_json::to_vec(value).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|_| AppError::DatabaseError("failed to encrypt credentials".to_owned()))?;

    Ok(serde_json::json!({
        "encrypted": true,
        "cipher": "AES-256-GCM",
        "nonce": STANDARD.encode(nonce_bytes),
        "payload": STANDARD.encode(ciphertext)
    }))
}

pub fn decrypt_json(key: &[u8; 32], value: &Value) -> Result<Value, AppError> {
    let nonce = value
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::DatabaseError("credential nonce is missing".to_owned()))?;
    let payload = value
        .get("payload")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::DatabaseError("credential payload is missing".to_owned()))?;
    let nonce = STANDARD
        .decode(nonce)
        .map_err(|_| AppError::DatabaseError("credential nonce is invalid".to_owned()))?;
    let payload = STANDARD
        .decode(payload)
        .map_err(|_| AppError::DatabaseError("credential payload is invalid".to_owned()))?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::DatabaseError("invalid encryption key".to_owned()))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), payload.as_ref())
        .map_err(|_| AppError::DatabaseError("failed to decrypt credentials".to_owned()))?;

    serde_json::from_slice(&plaintext).map_err(|error| AppError::DatabaseError(error.to_string()))
}

fn decode_hex(input: &str) -> Result<Vec<u8>, ()> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);

    if normalized.len() % 2 != 0 {
        return Err(());
    }

    let mut bytes = Vec::with_capacity(normalized.len() / 2);

    for pair in normalized.as_bytes().chunks_exact(2) {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }

    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}
