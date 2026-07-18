use chrono::Utc;
use serde_json::Value;

use crate::{
    error::AppError,
    markets::types::{
        MarketOutcomeResponse, MarketResponse, MarketTradingMetadata, OrderBookLevel,
        OrderBookResponse,
    },
    providers::{
        polymarket::dto::PolymarketMarket,
        registry::{ResolvedInstrument, ResolvedMarket},
        types::{Chain, ProviderId},
    },
};

pub(crate) fn normalize_market(raw: PolymarketMarket) -> Result<MarketResponse, AppError> {
    let id = text(&raw.id).ok_or_else(|| {
        validation(
            "PROVIDER_MARKET_INVALID",
            "provider market identity is missing",
        )
    })?;
    let title = text(&raw.question)
        .or_else(|| text(&raw.title))
        .ok_or_else(|| {
            validation(
                "PROVIDER_MARKET_INVALID",
                "provider market title is missing",
            )
        })?;

    let labels = string_array(&raw.outcomes);
    let instrument_ids = string_array(&raw.clob_token_ids);
    let prices = number_array(&raw.outcome_prices);
    let outcomes = labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| MarketOutcomeResponse {
            id: instrument_ids.get(index).cloned(),
            label,
            price: prices.get(index).copied(),
        })
        .collect();

    let chain = Chain::Polygon;
    Ok(MarketResponse {
        id,
        provider: ProviderId::Polymarket,
        chain,
        chain_id: chain.id(),
        title,
        description: text(&raw.description),
        category: text(&raw.category),
        image_url: text(&raw.image).or_else(|| text(&raw.icon)),
        external_url: text(&raw.url),
        active: raw.active.unwrap_or(false) && !raw.archived.unwrap_or(false),
        closed: raw.closed.unwrap_or(false),
        accepting_orders: raw.accepting_orders.unwrap_or(false),
        start_at: text(&raw.start_date),
        end_at: text(&raw.end_date),
        volume: number(&raw.volume_num).or_else(|| number(&raw.volume)),
        liquidity: number(&raw.liquidity_num).or_else(|| number(&raw.liquidity)),
        best_bid: number(&raw.best_bid),
        best_ask: number(&raw.best_ask),
        last_trade_price: number(&raw.last_trade_price),
        price_change_24h: number(&raw.one_day_price_change),
        outcomes,
        trading: MarketTradingMetadata {
            minimum_order_size: number(&raw.order_min_size),
            minimum_tick_size: number(&raw.order_price_min_tick_size),
            negative_risk: raw.neg_risk.unwrap_or(false),
        },
    })
}

pub(crate) fn resolve_market(
    raw: PolymarketMarket,
    requested_market_id: &str,
) -> Result<ResolvedMarket, AppError> {
    let requested_market_id = required(requested_market_id, "market id is required")?;
    let market = normalize_market(raw)?;

    if market.id != requested_market_id {
        return Err(validation(
            "PROVIDER_MARKET_MISMATCH",
            "provider market does not match the requested market",
        ));
    }

    Ok(ResolvedMarket {
        chain: Chain::Polygon,
        market_id: market.id.clone(),
        provider: ProviderId::Polymarket,
        title: market.title.clone(),
        market,
    })
}

pub(crate) fn resolve_instrument(
    market: &ResolvedMarket,
    requested_instrument_id: &str,
    requested_outcome: &str,
) -> Result<ResolvedInstrument, AppError> {
    let requested_instrument_id = required(requested_instrument_id, "outcome id is required")?;
    let requested_outcome = required(requested_outcome, "outcome is required")?;
    let outcome = market
        .market
        .outcome(&requested_instrument_id, &requested_outcome)
        .ok_or_else(|| {
            validation(
                "PROVIDER_INSTRUMENT_MISMATCH",
                "requested outcome id and label do not belong to the provider market",
            )
        })?;

    Ok(ResolvedInstrument {
        chain: market.chain,
        market_id: market.market_id.clone(),
        market_title: market.title.clone(),
        outcome: outcome.label.clone(),
        provider: market.provider,
        token_id: requested_instrument_id,
    })
}

pub(crate) fn resolve_order_book_outcome(
    market: &ResolvedMarket,
    requested_outcome_id: &str,
) -> Result<ResolvedInstrument, AppError> {
    let requested_outcome_id = required(requested_outcome_id, "outcome id is required")?;
    let outcome = market
        .market
        .outcome_by_id(&requested_outcome_id)
        .ok_or_else(|| {
            validation(
                "PROVIDER_OUTCOME_MISMATCH",
                "requested outcome does not belong to the provider market",
            )
        })?;

    Ok(ResolvedInstrument {
        chain: market.chain,
        market_id: market.market_id.clone(),
        market_title: market.title.clone(),
        outcome: outcome.label.clone(),
        provider: market.provider,
        token_id: requested_outcome_id,
    })
}

pub(crate) fn normalize_order_book(
    market_id: &str,
    outcome: &str,
    outcome_id: &str,
    payload: Value,
) -> OrderBookResponse {
    let mut bids = levels_from_payload(&payload, "bids");
    let mut asks = levels_from_payload(&payload, "asks");

    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| a.price.total_cmp(&b.price));

    let max_usd = bids
        .iter()
        .chain(asks.iter())
        .map(|level| level.usd)
        .fold(0.0, f64::max);
    if max_usd > 0.0 {
        for level in bids.iter_mut().chain(asks.iter_mut()) {
            level.depth_percent = ((level.usd / max_usd) * 100.0).clamp(0.0, 100.0);
        }
    }

    let best_bid = bids.first().map(|level| level.price);
    let best_ask = asks.first().map(|level| level.price);
    let spread = best_bid
        .zip(best_ask)
        .map(|(bid, ask)| (ask - bid).max(0.0));

    OrderBookResponse {
        provider: ProviderId::Polymarket,
        chain: Chain::Polygon,
        chain_id: Chain::Polygon.id(),
        market_id: market_id.to_owned(),
        outcome_id: outcome_id.to_owned(),
        outcome: outcome.to_owned(),
        asks,
        best_ask,
        best_bid,
        bids,
        last_traded: number_from_keys(
            &payload,
            &["last_traded", "lastTradePrice", "last_trade_price"],
        ),
        spread,
        updated_at: Utc::now().to_rfc3339(),
    }
}

fn levels_from_payload(payload: &Value, key: &str) -> Vec<OrderBookLevel> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(level_from_value).collect())
        .unwrap_or_default()
}

fn level_from_value(value: &Value) -> Option<OrderBookLevel> {
    let price = number_from_keys(value, &["price", "p"])?;
    let shares = number_from_keys(value, &["size", "shares", "s"])?;
    if !price.is_finite() || !shares.is_finite() || price <= 0.0 || shares <= 0.0 {
        return None;
    }

    Some(OrderBookLevel {
        depth_percent: 0.0,
        price,
        shares,
        usd: price * shares,
    })
}

fn required(value: &str, message: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(message.to_owned()));
    }
    Ok(value.to_owned())
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_owned()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn string_array(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items.iter().filter_map(text).collect(),
        Value::String(value) => serde_json::from_str::<Vec<Value>>(value)
            .map(|items| items.iter().filter_map(text).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn number_array(value: &Value) -> Vec<f64> {
    match value {
        Value::Array(items) => items.iter().filter_map(number).collect(),
        Value::String(value) => serde_json::from_str::<Vec<Value>>(value)
            .map(|items| items.iter().filter_map(number).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn number_from_keys(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| number(value.get(*key)?))
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn validation(code: &'static str, message: &'static str) -> AppError {
    AppError::ProviderValidation {
        code,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::providers::{polymarket::dto::PolymarketMarket, types::Chain};

    use super::{normalize_order_book, resolve_instrument, resolve_market};

    fn market() -> PolymarketMarket {
        serde_json::from_value(json!({
            "id": "market-1",
            "conditionId": "condition-1",
            "question": "Will it happen?",
            "outcomes": "[\"Yes\",\"No\"]",
            "outcomePrices": "[\"0.6\",\"0.4\"]",
            "clobTokenIds": "[\"yes-token\",\"no-token\"]",
            "volumeNum": 125.5,
            "enableOrderBook": true
        }))
        .unwrap()
    }

    #[test]
    fn normalizes_market_and_resolves_authoritative_identity() {
        let market = resolve_market(market(), "market-1").unwrap();
        let instrument = resolve_instrument(&market, "yes-token", "YES").unwrap();

        assert_eq!(market.market.chain, Chain::Polygon);
        assert_eq!(market.market.chain_id.value(), 137);
        assert_eq!(market.market.volume, Some(125.5));
        assert_eq!(market.market.outcomes[0].price, Some(0.6));
        assert_eq!(instrument.market_title, "Will it happen?");
        assert_eq!(instrument.outcome, "Yes");
        assert_eq!(instrument.chain.id().value(), 137);
    }

    #[test]
    fn rejects_market_instrument_and_outcome_mismatches() {
        assert!(resolve_market(market(), "market-2").is_err());
        let resolved = resolve_market(market(), "market-1").unwrap();
        assert!(resolve_instrument(&resolved, "no-token", "YES").is_err());
        assert!(resolve_instrument(&resolved, "yes-token", "MAYBE").is_err());
    }

    #[test]
    fn normalizes_and_sorts_order_book_levels() {
        let book = normalize_order_book(
            "market-1",
            "Yes",
            "yes-token",
            json!({
                "bids": [
                    {"price": "0.40", "size": "10"},
                    {"price": "0.60", "size": "5"}
                ],
                "asks": [
                    {"price": "0.80", "size": "3"},
                    {"price": "0.65", "size": "4"}
                ]
            }),
        );

        assert_eq!(book.bids[0].price, 0.60);
        assert_eq!(book.asks[0].price, 0.65);
        assert_eq!(book.best_bid, Some(0.60));
        assert_eq!(book.best_ask, Some(0.65));
        assert!((book.spread.unwrap() - 0.05).abs() < f64::EPSILON);
    }
}
