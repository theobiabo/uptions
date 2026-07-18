use std::{collections::HashMap, time::Duration};

use futures_util::{SinkExt, StreamExt};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, sea_query::OnConflict};
use serde_json::{Value, json};
use tokio::{
    task::JoinHandle,
    time::{MissedTickBehavior, interval, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    db::Db,
    entities::{polymarket_user_event, trade_intent, user, venue_connection},
    libs::{
        credentials::{decrypt_json, parse_encryption_key},
        wallet::{normalize_wallet_address, same_wallet},
    },
    polymarket::dto::{PolymarketApiCredentials, PolymarketUserEvent},
    trades::dto::TradeIntentStatus,
};

struct WorkerHandle {
    version: String,
    task: JoinHandle<()>,
}

pub struct PolymarketUserStreamSupervisor {
    credential_encryption_key: [u8; 32],
    db: Db,
    ws_url: String,
}

impl PolymarketUserStreamSupervisor {
    pub fn new(db: Db, credential_encryption_key: String, ws_url: String) -> Self {
        Self {
            credential_encryption_key: parse_encryption_key(&credential_encryption_key)
                .expect("CREDENTIAL_ENCRYPTION_KEY must resolve to 32 bytes"),
            db,
            ws_url,
        }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(self) {
        let mut workers: HashMap<String, WorkerHandle> = HashMap::new();
        loop {
            match venue_connection::Entity::find()
                .filter(venue_connection::Column::Venue.eq("polymarket"))
                .filter(venue_connection::Column::Enabled.eq(true))
                .filter(venue_connection::Column::Status.eq("active"))
                .all(&self.db)
                .await
            {
                Ok(connections) => {
                    let active_ids = connections
                        .iter()
                        .map(|connection| connection.id.clone())
                        .collect::<Vec<_>>();
                    let removed = workers
                        .keys()
                        .filter(|id| !active_ids.contains(id))
                        .cloned()
                        .collect::<Vec<_>>();
                    for id in removed {
                        if let Some(worker) = workers.remove(&id) {
                            worker.task.abort();
                        }
                    }
                    for connection in connections {
                        let version = connection.updated_at.to_rfc3339();
                        let restart = workers.get(&connection.id).is_none_or(|worker| {
                            worker.version != version || worker.task.is_finished()
                        });
                        if !restart {
                            continue;
                        }
                        if let Some(worker) = workers.remove(&connection.id) {
                            worker.task.abort();
                        }
                        let user_wallet = match user::Entity::find_by_id(&connection.user_id)
                            .one(&self.db)
                            .await
                        {
                            Ok(Some(user)) => user.primary_wallet_address,
                            Ok(None) => None,
                            Err(error) => {
                                error!(
                                    connection_id = %connection.id,
                                    error = %error,
                                    "failed to load user for Polymarket stream"
                                );
                                continue;
                            }
                        };
                        let credentials = match stream_credentials(
                            &self.credential_encryption_key,
                            &connection,
                            user_wallet.as_deref(),
                        ) {
                            Ok(credentials) => credentials,
                            Err(message) => {
                                warn!(
                                    connection_id = %connection.id,
                                    reason = %message,
                                    "Polymarket user stream connection is not eligible"
                                );
                                continue;
                            }
                        };
                        let db = self.db.clone();
                        let ws_url = self.ws_url.clone();
                        let connection_id = connection.id.clone();
                        let user_id = connection.user_id.clone();
                        let task = tokio::spawn(async move {
                            run_connection(db, ws_url, connection_id, user_id, credentials).await;
                        });
                        workers.insert(connection.id, WorkerHandle { version, task });
                    }
                }
                Err(error) => {
                    error!(error = %error, "failed to supervise Polymarket user streams");
                }
            }
            sleep(Duration::from_secs(5)).await;
        }
    }
}

fn stream_credentials(
    encryption_key: &[u8; 32],
    connection: &venue_connection::Model,
    user_wallet: Option<&str>,
) -> Result<PolymarketApiCredentials, String> {
    let user_wallet = user_wallet.ok_or_else(|| "connected wallet is missing".to_owned())?;
    if !same_wallet(user_wallet, &connection.account_identifier) {
        return Err("connection account does not match connected wallet".to_owned());
    }
    let config =
        decrypt_json(encryption_key, &connection.config).map_err(|error| error.to_string())?;
    let signature_type = config
        .get("signatureType")
        .and_then(|value| match value {
            Value::Number(value) => value.as_i64().and_then(|value| i32::try_from(value).ok()),
            Value::String(value) => value.parse().ok(),
            _ => None,
        })
        .ok_or_else(|| "signature type is missing".to_owned())?;
    if signature_type != 0 {
        return Err("private beta supports EOA connections only".to_owned());
    }
    let funder = config
        .get("funder")
        .and_then(Value::as_str)
        .ok_or_else(|| "funder is missing".to_owned())?;
    let funder = normalize_wallet_address(funder).map_err(|error| error.to_string())?;
    if !same_wallet(&funder, &connection.account_identifier) {
        return Err("EOA funder and account do not match".to_owned());
    }
    Ok(PolymarketApiCredentials {
        address: normalize_wallet_address(&connection.account_identifier)
            .map_err(|error| error.to_string())?,
        funder,
        signature_type,
        api_key: required_config(&config, &["apiKey", "api_key", "key"])?,
        secret: required_config(&config, &["secret"])?,
        passphrase: required_config(&config, &["passphrase"])?,
    })
}

fn required_config(value: &Value, keys: &[&str]) -> Result<String, String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "credentials are incomplete".to_owned())
}

async fn run_connection(
    db: Db,
    ws_url: String,
    connection_id: String,
    user_id: String,
    credentials: PolymarketApiCredentials,
) {
    let mut backoff_seconds = 1_u64;
    loop {
        match connect_async(&ws_url).await {
            Ok((socket, _)) => {
                info!(connection_id = %connection_id, "Polymarket user stream connected");
                backoff_seconds = 1;
                if let Err(error) =
                    consume_connection(&db, &connection_id, &user_id, &credentials, socket).await
                {
                    warn!(
                        connection_id = %connection_id,
                        error = %error,
                        "Polymarket user stream disconnected"
                    );
                }
            }
            Err(error) => {
                warn!(
                    connection_id = %connection_id,
                    error = %error,
                    "Polymarket user stream connection failed"
                );
            }
        }
        sleep(Duration::from_secs(backoff_seconds)).await;
        backoff_seconds = (backoff_seconds * 2).min(30);
    }
}

async fn consume_connection<S>(
    db: &Db,
    connection_id: &str,
    user_id: &str,
    credentials: &PolymarketApiCredentials,
    socket: tokio_tungstenite::WebSocketStream<S>,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut writer, mut reader) = socket.split();
    let subscription = json!({
        "auth": {
            "apiKey": credentials.api_key,
            "secret": credentials.secret,
            "passphrase": credentials.passphrase
        },
        "type": "user"
    });
    writer
        .send(Message::Text(subscription.to_string().into()))
        .await
        .map_err(|error| error.to_string())?;
    let mut heartbeat = interval(Duration::from_secs(10));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                writer
                    .send(Message::Text("PING".into()))
                    .await
                    .map_err(|error| error.to_string())?;
            }
            message = reader.next() => {
                match message {
                    Some(Ok(Message::Text(text))) if text.as_str() == "PONG" => {}
                    Some(Ok(Message::Text(text))) => {
                        let payload: Value = serde_json::from_str(text.as_str())
                            .map_err(|error| error.to_string())?;
                        for event in event_values(payload) {
                            process_event(db, connection_id, user_id, event)
                                .await
                                .map_err(|error| error.to_string())?;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        writer.send(Message::Pong(payload)).await.map_err(|error| error.to_string())?;
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(error.to_string()),
                }
            }
        }
    }
}

fn event_values(payload: Value) -> Vec<Value> {
    match payload {
        Value::Array(values) => values,
        Value::Object(_) => vec![payload],
        _ => Vec::new(),
    }
}

async fn process_event(
    db: &Db,
    connection_id: &str,
    user_id: &str,
    payload: Value,
) -> Result<(), sea_orm::DbErr> {
    let modeled = match serde_json::from_value::<PolymarketUserEvent>(payload.clone()) {
        Ok(modeled) => modeled,
        Err(_) => return Ok(()),
    };
    let event_kind = modeled.kind().to_owned();
    let provider_event_id = payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if provider_event_id.is_empty() {
        return Ok(());
    }
    let status = event_status(&payload);
    let order_ids = event_order_ids(&event_kind, &payload);
    let trades = if order_ids.is_empty() {
        Vec::new()
    } else {
        trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::ProviderOrderId.is_in(order_ids.clone()))
            .all(db)
            .await?
    };
    let provider_timestamp = payload
        .get("last_update")
        .or_else(|| payload.get("timestamp"))
        .and_then(value_text);
    let identity = event_identity(&event_kind, &provider_event_id, &payload);
    let provider_order_id = if event_kind == "order" {
        Some(provider_event_id.clone())
    } else {
        trades
            .first()
            .and_then(|trade| trade.provider_order_id.clone())
    };
    polymarket_user_event::Entity::insert(polymarket_user_event::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        user_id: Set(user_id.to_owned()),
        venue_connection_id: Set(connection_id.to_owned()),
        trade_intent_id: Set(trades.first().map(|trade| trade.id.clone())),
        event_kind: Set(event_kind.clone()),
        provider_event_id: Set(provider_event_id.clone()),
        event_identity: Set(identity),
        provider_order_id: Set(provider_order_id),
        provider_trade_id: Set((event_kind == "trade").then_some(provider_event_id)),
        status: Set(status.clone()),
        market_id: Set(payload.get("market").and_then(value_text)),
        token_id: Set(payload.get("asset_id").and_then(value_text)),
        provider_timestamp: Set(provider_timestamp),
        payload: Set(payload.clone()),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(polymarket_user_event::Column::EventIdentity)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;

    for trade in trades {
        apply_event_status(db, &trade, &event_kind, &payload).await?;
    }
    Ok(())
}

fn event_order_ids(event_kind: &str, payload: &Value) -> Vec<String> {
    if event_kind == "order" {
        return payload
            .get("id")
            .and_then(Value::as_str)
            .map(|value| vec![value.to_owned()])
            .unwrap_or_default();
    }
    let mut order_ids = payload
        .get("taker_order_id")
        .and_then(Value::as_str)
        .map(|value| vec![value.to_owned()])
        .unwrap_or_default();
    if let Some(maker_orders) = payload.get("maker_orders").and_then(Value::as_array) {
        order_ids.extend(maker_orders.iter().filter_map(|order| {
            order
                .get("order_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }));
    }
    order_ids.sort();
    order_ids.dedup();
    order_ids
}

fn event_status(payload: &Value) -> Option<String> {
    payload
        .get("status")
        .or_else(|| payload.get("type"))
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_uppercase())
}

fn event_identity(event_kind: &str, provider_event_id: &str, payload: &Value) -> String {
    let event_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let timestamp = payload
        .get("last_update")
        .or_else(|| payload.get("timestamp"))
        .and_then(value_text)
        .unwrap_or_default();
    let bucket = payload
        .get("bucket_index")
        .and_then(value_text)
        .unwrap_or_default();
    let matched = payload
        .get("size_matched")
        .and_then(value_text)
        .unwrap_or_default();
    format!("{event_kind}:{provider_event_id}:{event_type}:{status}:{timestamp}:{bucket}:{matched}")
}

async fn apply_event_status(
    db: &Db,
    trade: &trade_intent::Model,
    event_kind: &str,
    payload: &Value,
) -> Result<(), sea_orm::DbErr> {
    let provider_status = event_status(payload).unwrap_or_default();
    let target = if event_kind == "trade" {
        match provider_status.as_str() {
            "MATCHED" => Some(TradeIntentStatus::Matched),
            "MINED" => Some(TradeIntentStatus::Mined),
            "CONFIRMED" => Some(TradeIntentStatus::Filled),
            "RETRYING" => Some(TradeIntentStatus::Retrying),
            "FAILED" => Some(TradeIntentStatus::Failed),
            _ => None,
        }
    } else if provider_status == "CANCELLATION" || provider_status.contains("CANCEL") {
        Some(TradeIntentStatus::Cancelled)
    } else if payload
        .get("size_matched")
        .and_then(value_text)
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|value| value > 0.0)
    {
        Some(TradeIntentStatus::PartiallyFilled)
    } else {
        None
    };
    let Some(target) = target else {
        return Ok(());
    };
    let allowed = allowed_previous_statuses(target);
    let now = chrono::Utc::now();
    let mut active = <trade_intent::ActiveModel as Default>::default();
    active.status = Set(target.as_str().to_owned());
    active.updated_at = Set(now.into());
    active.error = Set(if target == TradeIntentStatus::Failed {
        Some("Polymarket reported terminal trade failure".to_owned())
    } else {
        None
    });
    if target == TradeIntentStatus::Cancelled {
        active.cancelled_at = Set(Some(now.into()));
    }
    trade_intent::Entity::update_many()
        .set(active)
        .filter(trade_intent::Column::Id.eq(&trade.id))
        .filter(trade_intent::Column::Status.is_in(allowed))
        .exec(db)
        .await?;
    Ok(())
}

fn allowed_previous_statuses(target: TradeIntentStatus) -> Vec<String> {
    let statuses = match target {
        TradeIntentStatus::Matched => vec![
            TradeIntentStatus::Submitting,
            TradeIntentStatus::ReconciliationRequired,
            TradeIntentStatus::Submitted,
            TradeIntentStatus::PartiallyFilled,
            TradeIntentStatus::CancellationRequested,
        ],
        TradeIntentStatus::Mined => vec![
            TradeIntentStatus::Matched,
            TradeIntentStatus::Retrying,
            TradeIntentStatus::CancellationRequested,
        ],
        TradeIntentStatus::Retrying => vec![
            TradeIntentStatus::Matched,
            TradeIntentStatus::Mined,
            TradeIntentStatus::CancellationRequested,
        ],
        TradeIntentStatus::Filled | TradeIntentStatus::Failed => vec![
            TradeIntentStatus::Submitting,
            TradeIntentStatus::ReconciliationRequired,
            TradeIntentStatus::Submitted,
            TradeIntentStatus::PartiallyFilled,
            TradeIntentStatus::Matched,
            TradeIntentStatus::Mined,
            TradeIntentStatus::Retrying,
            TradeIntentStatus::CancellationRequested,
        ],
        TradeIntentStatus::Cancelled => vec![
            TradeIntentStatus::Submitted,
            TradeIntentStatus::PartiallyFilled,
            TradeIntentStatus::Matched,
            TradeIntentStatus::Retrying,
            TradeIntentStatus::CancellationRequested,
        ],
        TradeIntentStatus::PartiallyFilled => vec![
            TradeIntentStatus::Submitted,
            TradeIntentStatus::Matched,
            TradeIntentStatus::CancellationRequested,
        ],
        _ => Vec::new(),
    };
    statuses
        .into_iter()
        .map(|status| status.as_str().to_owned())
        .collect()
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{polymarket::dto::PolymarketUserEvent, trades::dto::TradeIntentStatus};

    use super::{allowed_previous_statuses, event_identity, event_order_ids};

    #[test]
    fn models_documented_order_and_trade_events() {
        let order: PolymarketUserEvent = serde_json::from_value(json!({
            "event_type": "order",
            "id": "order-1",
            "type": "PLACEMENT",
            "status": "LIVE"
        }))
        .unwrap();
        let trade: PolymarketUserEvent = serde_json::from_value(json!({
            "event_type": "trade",
            "id": "trade-1",
            "type": "TRADE",
            "status": "MATCHED"
        }))
        .unwrap();
        assert_eq!(order.kind(), "order");
        assert_eq!(trade.kind(), "trade");
    }

    #[test]
    fn trade_identity_changes_with_lifecycle_status() {
        let matched = json!({
            "id": "trade-1",
            "status": "MATCHED",
            "last_update": "1",
            "bucket_index": 0
        });
        let confirmed = json!({
            "id": "trade-1",
            "status": "CONFIRMED",
            "last_update": "2",
            "bucket_index": 0
        });
        assert_ne!(
            event_identity("trade", "trade-1", &matched),
            event_identity("trade", "trade-1", &confirmed)
        );
    }

    #[test]
    fn extracts_taker_and_maker_order_ids() {
        let event = json!({
            "taker_order_id": "taker",
            "maker_orders": [{"order_id": "maker"}]
        });
        assert_eq!(
            event_order_ids("trade", &event),
            vec!["maker".to_owned(), "taker".to_owned()]
        );
    }

    #[test]
    fn matched_cannot_overwrite_mined_or_filled() {
        let allowed = allowed_previous_statuses(TradeIntentStatus::Matched);
        assert!(!allowed.contains(&TradeIntentStatus::Mined.as_str().to_owned()));
        assert!(!allowed.contains(&TradeIntentStatus::Filled.as_str().to_owned()));
    }
}
