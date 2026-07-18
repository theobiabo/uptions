use serde_json::Value;

use crate::{
    config::AppConfig,
    error::AppError,
    markets::types::{MarketListQuery, MarketPageResponse, MarketResponse, OrderBookResponse},
    providers::{
        polymarket::{
            PolymarketAdapter,
            client::PolymarketSubmissionError,
            credentials::PolymarketApiCredentials,
            dto::{PolymarketSignedOrderPayload, PolymarketTokenMetadataResponse},
        },
        types::{Chain, ProviderCapability, ProviderId, ProviderResponse},
    },
};

#[derive(Clone, Debug)]
pub struct ResolvedMarket {
    pub chain: Chain,
    pub market_id: String,
    pub provider: ProviderId,
    pub market: MarketResponse,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInstrument {
    pub chain: Chain,
    pub market_id: String,
    pub market_title: String,
    pub outcome: String,
    pub provider: ProviderId,
    pub token_id: String,
}

#[derive(Clone, Debug)]
pub(crate) enum ProviderTradingCredentials {
    Polymarket(PolymarketApiCredentials),
}

#[derive(Clone)]
pub enum ProviderAdapter {
    Polymarket(PolymarketAdapter),
}

impl ProviderAdapter {
    pub const fn id(&self) -> ProviderId {
        match self {
            Self::Polymarket(_) => ProviderId::Polymarket,
        }
    }

    pub const fn chain(&self) -> Chain {
        match self {
            Self::Polymarket(_) => Chain::Polygon,
        }
    }

    pub const fn supports(&self, capability: ProviderCapability) -> bool {
        match self {
            Self::Polymarket(_) => matches!(
                capability,
                ProviderCapability::MarketData
                    | ProviderCapability::Trading
                    | ProviderCapability::VenueConnection
                    | ProviderCapability::UserStream
                    | ProviderCapability::Automations
            ),
        }
    }

    pub fn capabilities(&self) -> Vec<ProviderCapability> {
        match self {
            Self::Polymarket(_) => vec![
                ProviderCapability::MarketData,
                ProviderCapability::Trading,
                ProviderCapability::VenueConnection,
                ProviderCapability::UserStream,
                ProviderCapability::Automations,
            ],
        }
    }
}

/// Finite, exhaustive dispatch keeps provider capabilities explicit at compile time.
#[derive(Clone)]
pub struct ProviderRegistry {
    polymarket: ProviderAdapter,
}

impl ProviderRegistry {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            polymarket: ProviderAdapter::Polymarket(PolymarketAdapter::new(config)),
        }
    }

    pub fn available(&self) -> Vec<ProviderId> {
        ProviderId::ALL
            .into_iter()
            .filter(|provider| self.adapter(*provider).is_ok())
            .collect()
    }

    pub fn catalog(&self) -> Vec<ProviderResponse> {
        self.available()
            .into_iter()
            .filter_map(|provider| self.describe(provider).ok())
            .collect()
    }

    pub fn describe(&self, provider: ProviderId) -> Result<ProviderResponse, AppError> {
        let adapter = self.adapter(provider)?;
        Ok(ProviderResponse::new(
            provider,
            adapter.chain(),
            adapter.capabilities(),
        ))
    }

    pub fn adapter(&self, provider: ProviderId) -> Result<&ProviderAdapter, AppError> {
        let adapter = match provider {
            ProviderId::Polymarket => &self.polymarket,
        };
        debug_assert_eq!(adapter.id(), provider);
        Ok(adapter)
    }

    pub fn require_capability(
        &self,
        provider: ProviderId,
        capability: ProviderCapability,
    ) -> Result<&ProviderAdapter, AppError> {
        let adapter = self.adapter(provider)?;
        if !adapter.supports(capability) {
            return Err(AppError::BadRequest(format!(
                "provider {} does not support {capability:?}",
                provider.route_value()
            )));
        }
        Ok(adapter)
    }

    pub fn chain(&self, provider: ProviderId) -> Result<Chain, AppError> {
        Ok(self.adapter(provider)?.chain())
    }

    pub async fn fetch_markets(
        &self,
        provider: ProviderId,
        query: &MarketListQuery,
    ) -> Result<MarketPageResponse, AppError> {
        match self.require_capability(provider, ProviderCapability::MarketData)? {
            ProviderAdapter::Polymarket(adapter) => adapter.fetch_markets(query).await,
        }
    }

    pub async fn fetch_market(
        &self,
        provider: ProviderId,
        market_id: &str,
    ) -> Result<MarketResponse, AppError> {
        Ok(self.resolve_market(provider, market_id).await?.market)
    }

    pub async fn resolve_market(
        &self,
        provider: ProviderId,
        market_id: &str,
    ) -> Result<ResolvedMarket, AppError> {
        match self.require_capability(provider, ProviderCapability::MarketData)? {
            ProviderAdapter::Polymarket(adapter) => adapter.resolve_market(market_id).await,
        }
    }

    pub async fn resolve_instrument(
        &self,
        provider: ProviderId,
        market_id: &str,
        token_id: &str,
        outcome: &str,
    ) -> Result<(ResolvedInstrument, PolymarketTokenMetadataResponse), AppError> {
        match self.require_capability(provider, ProviderCapability::Trading)? {
            ProviderAdapter::Polymarket(adapter) => {
                adapter
                    .resolve_instrument(market_id, token_id, outcome)
                    .await
            }
        }
    }

    pub async fn fetch_order_book(
        &self,
        provider: ProviderId,
        market_id: &str,
        outcome_id: &str,
    ) -> Result<OrderBookResponse, AppError> {
        match self.require_capability(provider, ProviderCapability::MarketData)? {
            ProviderAdapter::Polymarket(adapter) => {
                adapter.fetch_order_book(market_id, outcome_id).await
            }
        }
    }

    pub(crate) async fn submit_signed_order(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        payload: &PolymarketSignedOrderPayload,
    ) -> Result<Value, PolymarketSubmissionError> {
        match (self.adapter(provider), credentials) {
            (
                Ok(ProviderAdapter::Polymarket(adapter)),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.submit_signed_order(credentials, payload).await,
            _ => Err(PolymarketSubmissionError::Definite(
                "provider credentials do not match the stored order provider".to_owned(),
            )),
        }
    }

    pub(crate) async fn get_order(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.get_order(credentials, order_id).await,
        }
    }

    pub(crate) async fn get_trades(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        trade_id: &str,
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.get_trades(credentials, trade_id).await,
        }
    }

    pub(crate) async fn cancel_order(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.cancel_order(credentials, order_id).await,
        }
    }

    pub(crate) async fn cancel_orders(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        order_ids: &[String],
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.cancel_orders(credentials, order_ids).await,
        }
    }

    pub(crate) async fn cancel_all_orders(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => adapter.cancel_all_orders(credentials).await,
        }
    }

    pub(crate) async fn cancel_market_orders(
        &self,
        provider: ProviderId,
        credentials: &ProviderTradingCredentials,
        market_id: &str,
        token_id: &str,
    ) -> Result<Value, AppError> {
        match (self.adapter(provider)?, credentials) {
            (
                ProviderAdapter::Polymarket(adapter),
                ProviderTradingCredentials::Polymarket(credentials),
            ) => {
                adapter
                    .cancel_market_orders(credentials, market_id, token_id)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::AppConfig,
        providers::types::{ProviderCapability, ProviderId},
    };

    use super::{ProviderAdapter, ProviderRegistry};

    fn config() -> AppConfig {
        AppConfig {
            server_address: "127.0.0.1:0".to_owned(),
            database_url: "postgres://unused".to_owned(),
            credential_encryption_key:
                "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            app_base_url: "http://localhost:5173".to_owned(),
            polymarket_clob_host: "https://clob.polymarket.com".to_owned(),
            polymarket_gamma_host: "https://gamma-api.polymarket.com".to_owned(),
            polymarket_user_ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/user".to_owned(),
            environment: "test".to_owned(),
            swagger_enabled: false,
            cors_allowed_origins: vec![],
            request_body_limit_bytes: 1_048_576,
            concurrency_limit: 256,
            public_rate_limit_per_minute: 120,
            auth_rate_limit_per_minute: 10,
            external_rate_limit_per_minute: 60,
        }
    }

    #[test]
    fn registry_dispatch_is_finite_and_exhaustive() {
        let registry = ProviderRegistry::new(&config());
        assert_eq!(registry.available(), vec![ProviderId::Polymarket]);
        match registry.adapter(ProviderId::Polymarket).unwrap() {
            ProviderAdapter::Polymarket(adapter) => {
                let _ = adapter;
            }
        }
        assert!(
            registry
                .require_capability(ProviderId::Polymarket, ProviderCapability::Trading)
                .is_ok()
        );
    }
}
