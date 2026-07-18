use serde::Deserialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Debug)]
pub struct PolymarketApiCredentials {
    pub address: String,
    pub funder: String,
    pub signature_type: i32,
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum PolymarketSignatureType {
    Eoa = 0,
    PolyProxy = 1,
    GnosisSafe = 2,
}

impl PolymarketSignatureType {
    pub const fn value(self) -> i32 {
        self as i32
    }
}

impl<'de> Deserialize<'de> for PolymarketSignatureType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match i32::deserialize(deserializer)? {
            0 => Ok(Self::Eoa),
            1 => Ok(Self::PolyProxy),
            2 => Ok(Self::GnosisSafe),
            _ => Err(serde::de::Error::custom(
                "signature_type must be 0 (EOA), 1 (POLY_PROXY), or 2 (GNOSIS_SAFE)",
            )),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConnectPolymarketRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub account_identifier: Option<String>,
    #[schema(example = "3e8f4f1a-3be4-43ef-a9b3-df6d83cc66cc")]
    pub api_key: String,
    #[schema(example = "base64-secret-value")]
    pub secret: String,
    #[schema(example = "polymarket-passphrase")]
    pub passphrase: String,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub funder: Option<String>,
    #[schema(value_type = i32, example = 0, minimum = 0, maximum = 2)]
    pub signature_type: Option<PolymarketSignatureType>,
    pub limits: Option<Value>,
    pub permissions: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::PolymarketSignatureType;

    #[test]
    fn parses_only_documented_private_beta_signature_types() {
        assert_eq!(
            serde_json::from_str::<PolymarketSignatureType>("0").unwrap(),
            PolymarketSignatureType::Eoa
        );
        assert_eq!(
            serde_json::from_str::<PolymarketSignatureType>("1").unwrap(),
            PolymarketSignatureType::PolyProxy
        );
        assert_eq!(
            serde_json::from_str::<PolymarketSignatureType>("2").unwrap(),
            PolymarketSignatureType::GnosisSafe
        );
        assert!(serde_json::from_str::<PolymarketSignatureType>("3").is_err());
    }
}
