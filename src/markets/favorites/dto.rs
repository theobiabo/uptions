use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::providers::types::ProviderId;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct MarketFavoritesQuery {
    #[param(example = 50, minimum = 1, maximum = 100)]
    pub limit: Option<u64>,
    #[param(example = "96b5ce61-0c67-46a0-9925-bfbe3af0aa82")]
    pub before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketFavoriteStatusResponse {
    pub provider: ProviderId,
    #[schema(example = "123456")]
    pub market_id: String,
    pub favorited: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketFavoritesPageResponse {
    pub provider: ProviderId,
    pub market_ids: Vec<String>,
    #[schema(example = "96b5ce61-0c67-46a0-9925-bfbe3af0aa82")]
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{MarketFavoriteStatusResponse, MarketFavoritesPageResponse};

    #[test]
    fn favorite_payloads_do_not_fabricate_market_metadata() {
        let status = serde_json::to_value(MarketFavoriteStatusResponse {
            provider: crate::providers::types::ProviderId::Polymarket,
            market_id: "market-1".to_owned(),
            favorited: true,
        })
        .unwrap();
        let page = serde_json::to_value(MarketFavoritesPageResponse {
            provider: crate::providers::types::ProviderId::Polymarket,
            market_ids: vec!["market-1".to_owned()],
            next_cursor: None,
        })
        .unwrap();

        assert_eq!(status.as_object().unwrap().len(), 3);
        assert_eq!(page.as_object().unwrap().len(), 3);
        assert_eq!(status["provider"], "POLYMARKET");
        assert_eq!(page["provider"], "POLYMARKET");
        assert!(status.get("title").is_none());
        assert!(page.get("markets").is_none());
    }
}
