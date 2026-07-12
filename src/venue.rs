use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportedChain {
    Polygon,
}

impl SupportedChain {
    pub fn chain_id(self) -> u64 {
        match self {
            Self::Polygon => 137,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Polygon => "Polygon",
        }
    }

    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Polygon => "POLYGON",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportedVenue {
    Polymarket,
}

impl SupportedVenue {
    pub fn all() -> Vec<Self> {
        vec![Self::Polymarket]
    }

    pub fn available(self) -> bool {
        match self {
            Self::Polymarket => true,
        }
    }

    pub fn chain(self) -> SupportedChain {
        match self {
            Self::Polymarket => SupportedChain::Polygon,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Polymarket => "Polymarket",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Polymarket => "Prediction markets on Polygon.",
        }
    }

    pub fn image_key(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
        }
    }

    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Polymarket => "POLYMARKET",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "POLYMARKET" => Some(Self::Polymarket),
            _ => None,
        }
    }
}
