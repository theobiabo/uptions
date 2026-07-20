use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ToSchema)]
#[schema(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderId {
    Polymarket,
}

pub const DEFAULT_PROVIDER: ProviderId = ProviderId::Polymarket;

impl ProviderId {
    pub const ALL: [Self; 1] = [Self::Polymarket];

    pub const fn api_value(self) -> &'static str {
        match self {
            Self::Polymarket => "POLYMARKET",
        }
    }

    pub const fn storage_value(self) -> &'static str {
        match self {
            Self::Polymarket => "POLYMARKET",
        }
    }

    pub const fn route_value(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Polymarket => "Polymarket",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Polymarket => "Prediction markets on Polygon.",
        }
    }

    pub const fn image_key(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        Self::from_str(value).ok()
    }
}

impl Default for ProviderId {
    fn default() -> Self {
        DEFAULT_PROVIDER
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.route_value())
    }
}

impl FromStr for ProviderId {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "polymarket" => Ok(Self::Polymarket),
            _ => Err("provider is invalid or unavailable"),
        }
    }
}

impl Serialize for ProviderId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.api_value())
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ToSchema)]
#[schema(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Chain {
    Polygon,
}

impl Chain {
    pub const fn api_value(self) -> &'static str {
        match self {
            Self::Polygon => "POLYGON",
        }
    }

    pub const fn storage_value(self) -> &'static str {
        match self {
            Self::Polygon => "POLYGON",
        }
    }

    pub const fn route_value(self) -> &'static str {
        match self {
            Self::Polygon => "polygon",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Polygon => "Polygon",
        }
    }

    pub const fn id(self) -> ChainId {
        match self {
            Self::Polygon => ChainId(137),
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "POLYGON" => Some(Self::Polygon),
            _ => None,
        }
    }
}

impl Serialize for Chain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.api_value())
    }
}

impl<'de> Deserialize<'de> for Chain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_storage(&value).ok_or_else(|| de::Error::custom("chain is invalid"))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct ChainId(pub u64);

impl ChainId {
    pub const POLYGON: Self = Self(137);

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderCapability {
    MarketData,
    Trading,
    VenueConnection,
    UserStream,
    Automations,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProviderResponse {
    pub provider: ProviderId,
    pub id: String,
    pub label: String,
    pub description: String,
    pub image_key: String,
    pub available: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub chain: Chain,
    pub chain_id: ChainId,
    pub chain_label: String,
}

impl ProviderResponse {
    pub fn new(provider: ProviderId, chain: Chain, capabilities: Vec<ProviderCapability>) -> Self {
        Self {
            provider,
            id: provider.route_value().to_owned(),
            label: provider.label().to_owned(),
            description: provider.description().to_owned(),
            image_key: provider.image_key().to_owned(),
            available: true,
            capabilities,
            chain,
            chain_id: chain.id(),
            chain_label: chain.label().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chain, ChainId, DEFAULT_PROVIDER, ProviderId};

    #[test]
    fn canonical_provider_representations_are_explicit() {
        assert_eq!(DEFAULT_PROVIDER, ProviderId::Polymarket);
        assert_eq!(DEFAULT_PROVIDER.api_value(), "POLYMARKET");
        assert_eq!(DEFAULT_PROVIDER.storage_value(), "POLYMARKET");
        assert_eq!(DEFAULT_PROVIDER.route_value(), "polymarket");
        assert_eq!(DEFAULT_PROVIDER.label(), "Polymarket");
        assert_eq!(
            serde_json::to_string(&DEFAULT_PROVIDER).unwrap(),
            "\"POLYMARKET\""
        );
        assert_eq!(
            serde_json::from_str::<ProviderId>("\"polymarket\"").unwrap(),
            DEFAULT_PROVIDER
        );
    }

    #[test]
    fn polygon_identity_is_canonical() {
        assert_eq!(Chain::Polygon.id(), ChainId::POLYGON);
        assert_eq!(Chain::Polygon.id().value(), 137);
        assert_eq!(Chain::Polygon.storage_value(), "POLYGON");
        assert_eq!(Chain::Polygon.route_value(), "polygon");
    }
}
