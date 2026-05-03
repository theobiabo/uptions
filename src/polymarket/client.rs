use std::time::Duration;

use k256::{
    EncodedPoint,
    ecdsa::{RecoveryId, SigningKey},
};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;
use sha3::{Digest, Keccak256};

use crate::{
    config::AppConfig,
    error::AppError,
    polymarket::dto::{MarketsQuery, PolymarketAuthResponse},
};

const AUTH_MESSAGE: &str = "This message attests that I control the given wallet";
const EIP712_DOMAIN_TYPE: &str = "EIP712Domain(string name,string version,uint256 chainId)";
const CLOB_AUTH_TYPE: &str = "ClobAuth(address address,string timestamp,uint256 nonce,string message)";
const DOMAIN_NAME: &str = "ClobAuthDomain";
const DOMAIN_VERSION: &str = "1";

#[derive(Clone)]
pub struct PolymarketClient {
    http_client: Client,
    gamma_host: String,
    clob_host: String,
    chain_id: u64,
    private_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiCredentials {
    #[serde(rename = "apiKey")]
    api_key: String,
    secret: String,
    passphrase: String,
}

impl PolymarketClient {
    pub fn new(config: &AppConfig) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("polymarket http client should build");

        Self {
            http_client,
            gamma_host: config.polymarket_gamma_host.clone(),
            clob_host: config.polymarket_clob_host.clone(),
            chain_id: config.polymarket_chain_id,
            private_key: config.polymarket_private_key.clone(),
        }
    }

    pub async fn create_or_derive_api_key(
        &self,
        nonce: Option<u32>,
    ) -> Result<PolymarketAuthResponse, AppError> {
        let signing_key = self.signing_key()?;
        let address = signer_address(&signing_key);
        let timestamp = self.server_timestamp().await?;
        let nonce = nonce.unwrap_or(0);
        let l1_headers = build_l1_headers(&signing_key, &address, timestamp, nonce, self.chain_id)?;

        let create_response = self
            .http_client
            .post(format!("{}/auth/api-key", self.clob_host))
            .headers(l1_headers.clone())
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        let credentials = if create_response.status().is_success() {
            create_response
                .json::<ApiCredentials>()
                .await
                .map_err(|error| AppError::ExternalApiError(error.to_string()))?
        } else {
            let status = create_response.status();
            let body = create_response
                .text()
                .await
                .unwrap_or_else(|_| "unknown upstream error".to_owned());

            if status.is_client_error() && nonce_already_used(&body) {
                self.derive_api_key(&l1_headers).await?
            } else {
                return Err(AppError::ExternalApiError(body));
            }
        };

        Ok(PolymarketAuthResponse {
            address,
            api_key: credentials.api_key,
            secret: credentials.secret,
            passphrase: credentials.passphrase,
        })
    }

    pub async fn fetch_markets(&self, query: &MarketsQuery) -> Result<Value, AppError> {
        let response = self
            .http_client
            .get(format!("{}/markets", self.gamma_host))
            .query(query)
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        parse_json_response(response).await
    }

    async fn derive_api_key(&self, l1_headers: &HeaderMap) -> Result<ApiCredentials, AppError> {
        let response = self
            .http_client
            .get(format!("{}/auth/derive-api-key", self.clob_host))
            .headers(l1_headers.clone())
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        parse_json_response(response).await
    }

    async fn server_timestamp(&self) -> Result<i64, AppError> {
        let response = self
            .http_client
            .get(format!("{}/time", self.clob_host))
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read upstream response".to_owned());
            return Err(AppError::ExternalApiError(body));
        }

        response
            .json::<i64>()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))
    }

    fn signing_key(&self) -> Result<SigningKey, AppError> {
        let private_key = self.private_key.as_deref().ok_or_else(|| {
            AppError::ConfigurationError("POLYMARKET_PRIVATE_KEY is not configured".to_owned())
        })?;

        let secret_bytes = decode_hex(private_key)?;
        SigningKey::from_slice(&secret_bytes)
            .map_err(|_| AppError::ConfigurationError("invalid POLYMARKET_PRIVATE_KEY".to_owned()))
    }
}

async fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, AppError> {
    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read upstream response".to_owned());
        return Err(AppError::ExternalApiError(body));
    }

    response
        .json::<T>()
        .await
        .map_err(|error| AppError::ExternalApiError(error.to_string()))
}

fn build_l1_headers(
    signing_key: &SigningKey,
    address: &str,
    timestamp: i64,
    nonce: u32,
    chain_id: u64,
) -> Result<HeaderMap, AppError> {
    let signature = sign_clob_auth(signing_key, address, timestamp, nonce, chain_id)?;
    let mut headers = HeaderMap::new();

    headers.insert(
        HeaderName::from_static("poly-address"),
        HeaderValue::from_str(address).map_err(|_| AppError::InternalServerError)?,
    );
    headers.insert(
        HeaderName::from_static("poly-signature"),
        HeaderValue::from_str(&signature).map_err(|_| AppError::InternalServerError)?,
    );
    headers.insert(
        HeaderName::from_static("poly-timestamp"),
        HeaderValue::from_str(&timestamp.to_string())
            .map_err(|_| AppError::InternalServerError)?,
    );
    headers.insert(
        HeaderName::from_static("poly-nonce"),
        HeaderValue::from_str(&nonce.to_string()).map_err(|_| AppError::InternalServerError)?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    Ok(headers)
}

fn sign_clob_auth(
    signing_key: &SigningKey,
    address: &str,
    timestamp: i64,
    nonce: u32,
    chain_id: u64,
) -> Result<String, AppError> {
    let address_bytes = decode_hex(address)?;
    if address_bytes.len() != 20 {
        return Err(AppError::ConfigurationError(
            "derived Polymarket signer address is invalid".to_owned(),
        ));
    }

    let domain_separator = eip712_domain_separator(chain_id);
    let struct_hash = clob_auth_struct_hash(&address_bytes, timestamp, nonce);

    let mut payload = Vec::with_capacity(66);
    payload.extend_from_slice(&[0x19, 0x01]);
    payload.extend_from_slice(&domain_separator);
    payload.extend_from_slice(&struct_hash);

    let (signature, recovery_id) = signing_key
        .sign_digest_recoverable(Keccak256::new_with_prefix(payload))
        .map_err(|_| AppError::InternalServerError)?;

    Ok(serialize_signature(&signature.to_bytes(), recovery_id))
}

fn signer_address(signing_key: &SigningKey) -> String {
    let verifying_key = signing_key.verifying_key();
    let encoded_point: EncodedPoint = verifying_key.to_encoded_point(false);
    let public_key = encoded_point.as_bytes();
    let hash = keccak256(&public_key[1..]);
    format!("0x{}", encode_hex(&hash[12..]))
}

fn eip712_domain_separator(chain_id: u64) -> [u8; 32] {
    let mut encoded = Vec::with_capacity(128);
    encoded.extend_from_slice(&keccak256(EIP712_DOMAIN_TYPE.as_bytes()));
    encoded.extend_from_slice(&keccak256(DOMAIN_NAME.as_bytes()));
    encoded.extend_from_slice(&keccak256(DOMAIN_VERSION.as_bytes()));
    encoded.extend_from_slice(&u256_bytes(chain_id));
    keccak256(&encoded)
}

fn clob_auth_struct_hash(address: &[u8], timestamp: i64, nonce: u32) -> [u8; 32] {
    let mut encoded = Vec::with_capacity(160);
    encoded.extend_from_slice(&keccak256(CLOB_AUTH_TYPE.as_bytes()));
    encoded.extend_from_slice(&left_pad_32(address));
    encoded.extend_from_slice(&keccak256(timestamp.to_string().as_bytes()));
    encoded.extend_from_slice(&u256_bytes(u64::from(nonce)));
    encoded.extend_from_slice(&keccak256(AUTH_MESSAGE.as_bytes()));
    keccak256(&encoded)
}

fn nonce_already_used(body: &str) -> bool {
    body.contains("NONCE_ALREADY_USED") || body.to_ascii_lowercase().contains("already used")
}

fn serialize_signature(signature: &[u8], recovery_id: RecoveryId) -> String {
    let mut encoded = Vec::with_capacity(65);
    encoded.extend_from_slice(signature);
    encoded.push(recovery_id.to_byte().saturating_add(27));
    format!("0x{}", encode_hex(&encoded))
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn left_pad_32(bytes: &[u8]) -> [u8; 32] {
    let mut padded = [0_u8; 32];
    let start = 32 - bytes.len();
    padded[start..].copy_from_slice(bytes);
    padded
}

fn u256_bytes(value: u64) -> [u8; 32] {
    let mut encoded = [0_u8; 32];
    encoded[24..].copy_from_slice(&value.to_be_bytes());
    encoded
}

fn decode_hex(input: &str) -> Result<Vec<u8>, AppError> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);

    if normalized.len() % 2 != 0 {
        return Err(AppError::ConfigurationError(
            "hex values must have an even number of characters".to_owned(),
        ));
    }

    let mut bytes = Vec::with_capacity(normalized.len() / 2);
    let chars = normalized.as_bytes().chunks_exact(2);

    for pair in chars {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }

    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, AppError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AppError::ConfigurationError(
            "hex values may only contain 0-9, a-f, or A-F".to_owned(),
        )),
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
