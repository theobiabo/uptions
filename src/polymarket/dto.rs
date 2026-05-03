use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PolymarketAuthRequest {
    pub nonce: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct PolymarketAuthResponse {
    pub address: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct MarketsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub slug: Option<String>,
    pub id: Option<String>,
}
