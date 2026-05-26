use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use k256::{
    EncodedPoint,
    ecdsa::{RecoveryId, Signature, VerifyingKey},
};
use sha3::{Digest, Keccak256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    auth::dto::{AuthUserResponse, CreateChallengeResponse, VerifyChallengeResponse},
    error::AppError,
};

const CHALLENGE_TTL_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct AuthService {
    challenges: Arc<RwLock<HashMap<String, ChallengeRecord>>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
}

#[derive(Clone)]
struct ChallengeRecord {
    wallet_address: String,
    message: String,
    expires_at: u64,
}

#[derive(Clone)]
struct SessionRecord {
    wallet_address: String,
}

impl AuthService {
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_challenge(
        &self,
        wallet_address: &str,
    ) -> Result<CreateChallengeResponse, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let nonce = Uuid::new_v4().to_string();
        let expires_at = unix_timestamp() + CHALLENGE_TTL_SECONDS;
        let message = format!("Sign in to Uptions\nAddress: {wallet_address}\nNonce: {nonce}");

        let record = ChallengeRecord {
            wallet_address: wallet_address.clone(),
            message: message.clone(),
            expires_at,
        };

        self.challenges
            .write()
            .await
            .insert(wallet_address.clone(), record);

        Ok(CreateChallengeResponse {
            wallet_address,
            nonce,
            message,
            expires_at,
        })
    }

    pub async fn verify_challenge(
        &self,
        wallet_address: &str,
        signature: &str,
    ) -> Result<VerifyChallengeResponse, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let challenge = self
            .challenges
            .write()
            .await
            .remove(&wallet_address)
            .ok_or_else(|| AppError::BadRequest("challenge not found for wallet".to_owned()))?;

        if challenge.expires_at < unix_timestamp() {
            return Err(AppError::BadRequest("challenge expired".to_owned()));
        }

        if challenge.wallet_address != wallet_address {
            return Err(AppError::BadRequest("challenge wallet mismatch".to_owned()));
        }

        let recovered_address = recover_wallet_address(&challenge.message, signature)?;
        if recovered_address != wallet_address {
            return Err(AppError::Unauthorized);
        }

        let access_token = Uuid::new_v4().to_string();
        self.sessions.write().await.insert(
            access_token.clone(),
            SessionRecord {
                wallet_address: wallet_address.clone(),
            },
        );

        Ok(VerifyChallengeResponse {
            access_token,
            token_type: "Bearer".to_owned(),
            user: AuthUserResponse {
                wallet_address,
                polymarket_linked: false,
            },
        })
    }

    pub async fn current_user(&self, access_token: &str) -> Result<AuthUserResponse, AppError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(access_token).ok_or(AppError::Unauthorized)?;

        Ok(AuthUserResponse {
            wallet_address: session.wallet_address.clone(),
            polymarket_linked: false,
        })
    }
}

fn recover_wallet_address(message: &str, signature: &str) -> Result<String, AppError> {
    let signature_bytes = decode_hex(signature).map_err(|_| AppError::Unauthorized)?;
    if signature_bytes.len() != 65 {
        return Err(AppError::Unauthorized);
    }

    let signature =
        Signature::try_from(&signature_bytes[..64]).map_err(|_| AppError::Unauthorized)?;
    let recovery_byte =
        normalize_recovery_byte(signature_bytes[64]).ok_or(AppError::Unauthorized)?;
    let recovery_id = RecoveryId::from_byte(recovery_byte).ok_or(AppError::Unauthorized)?;
    let digest = ethereum_message_digest(message);
    let verifying_key = VerifyingKey::recover_from_digest(digest, &signature, recovery_id)
        .map_err(|_| AppError::Unauthorized)?;

    Ok(verifying_key_to_address(&verifying_key))
}

fn normalize_recovery_byte(byte: u8) -> Option<u8> {
    match byte {
        27 | 28 => Some(byte - 27),
        0 | 1 => Some(byte),
        _ => None,
    }
}

fn ethereum_message_digest(message: &str) -> Keccak256 {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut payload = Vec::with_capacity(prefix.len() + message.len());
    payload.extend_from_slice(prefix.as_bytes());
    payload.extend_from_slice(message.as_bytes());
    Keccak256::new_with_prefix(payload)
}

fn verifying_key_to_address(verifying_key: &VerifyingKey) -> String {
    let encoded_point: EncodedPoint = verifying_key.to_encoded_point(false);
    let public_key = encoded_point.as_bytes();
    let hash = keccak256(&public_key[1..]);
    format!("0x{}", encode_hex(&hash[12..]))
}

fn normalize_wallet_address(wallet_address: &str) -> Result<String, AppError> {
    let decoded = decode_hex(wallet_address)
        .map_err(|_| AppError::BadRequest("invalid wallet address".to_owned()))?;

    if decoded.len() != 20 {
        return Err(AppError::BadRequest("invalid wallet address".to_owned()));
    }

    Ok(format!("0x{}", encode_hex(&decoded)))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
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

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}
