use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    db::Db,
    entities::{trade_intent, user, venue_connection},
    error::AppError,
    libs::{
        credentials::{decrypt_json, parse_encryption_key},
        wallet::{normalize_wallet_address, same_wallet},
    },
    polymarket::{
        client::PolymarketClient,
        dto::{PolymarketApiCredentials, PolymarketSignedOrderPayload},
    },
    trades::dto::{
        CreateTradeIntentRequest, CreateTradeIntentResponse, SubmitSignedTradeRequest,
        SubmitSignedTradeResponse, TradeIntentResponse, TradeIntentStatus, TradeOrderType,
    },
    venue::SupportedVenue,
};

#[derive(Clone)]
pub struct TradeService {
    credential_encryption_key: [u8; 32],
    db: Db,
    polymarket_client: PolymarketClient,
}

impl TradeService {
    pub fn new(
        db: Db,
        polymarket_client: PolymarketClient,
        credential_encryption_key: String,
    ) -> Self {
        Self {
            credential_encryption_key: parse_encryption_key(&credential_encryption_key)
                .expect("CREDENTIAL_ENCRYPTION_KEY must resolve to 32 bytes"),
            db,
            polymarket_client,
        }
    }

    pub async fn list(&self, user_id: &str) -> Result<Vec<TradeIntentResponse>, AppError> {
        let trades = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .order_by_desc(trade_intent::Column::UpdatedAt)
            .all(&self.db)
            .await?;

        Ok(trades.into_iter().map(trade_response).collect())
    }

    pub async fn get(
        &self,
        user_id: &str,
        trade_id: &str,
    ) -> Result<TradeIntentResponse, AppError> {
        Ok(trade_response(
            self.find_owned_trade(user_id, trade_id).await?,
        ))
    }

    pub async fn create_intent(
        &self,
        user_id: &str,
        payload: CreateTradeIntentRequest,
    ) -> Result<CreateTradeIntentResponse, AppError> {
        self.validate_readiness(user_id, payload.provider, &payload.wallet_address)
            .await?;
        validate_trade_payload(&payload)?;
        let chain = payload.provider.chain();
        let token_metadata = self
            .polymarket_client
            .fetch_token_metadata(&payload.token_id)
            .await?;
        let now = Utc::now();
        let model = trade_intent::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            automation_id: Set(clean_optional(payload.automation_id)),
            provider: Set(payload.provider.as_storage_value().to_owned()),
            chain: Set(chain.as_storage_value().to_owned()),
            chain_id: Set(chain.chain_id() as i64),
            market_id: Set(clean_required(&payload.market_id, "market id is required")?),
            market_title: Set(clean_required(
                &payload.market_title,
                "market title is required",
            )?),
            token_id: Set(clean_required(&payload.token_id, "token id is required")?),
            outcome: Set(clean_required(&payload.outcome, "outcome is required")?),
            side: Set(payload.side.as_str().to_owned()),
            order_type: Set(payload.order_type.as_str().to_owned()),
            execution_type: Set(payload.execution_type.as_str().to_owned()),
            amount: Set(payload.amount),
            price: Set(payload.price),
            wallet_address: Set(normalize_wallet_address(&payload.wallet_address)?),
            status: Set(TradeIntentStatus::PendingSignature.as_str().to_owned()),
            signed_order: Set(None),
            provider_response: Set(None),
            provider_order_id: Set(None),
            error: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            submitted_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        Ok(CreateTradeIntentResponse {
            trade: trade_response(model),
            token_metadata,
        })
    }

    pub async fn submit_signed_order(
        &self,
        user_id: &str,
        trade_id: &str,
        payload: SubmitSignedTradeRequest,
    ) -> Result<SubmitSignedTradeResponse, AppError> {
        let trade = self.find_owned_trade(user_id, trade_id).await?;

        if trade.status != TradeIntentStatus::PendingSignature.as_str()
            && trade.status != TradeIntentStatus::Failed.as_str()
        {
            return Err(AppError::Conflict(
                "trade is not awaiting signature".to_owned(),
            ));
        }

        let provider = SupportedVenue::from_storage_value(&trade.provider)
            .ok_or_else(|| AppError::BadRequest("trade provider is invalid".to_owned()))?;
        self.validate_readiness(user_id, provider, &trade.wallet_address)
            .await?;
        validate_signed_order(&payload.signed_order, &trade)?;
        let credentials = self
            .credentials(user_id, provider, &trade.wallet_address)
            .await?;
        let polymarket_payload = PolymarketSignedOrderPayload {
            defer_exec: payload.defer_exec,
            execution_type: payload.execution_type.as_str().to_owned(),
            post_only: payload.post_only,
            signed_order: payload.signed_order.clone(),
        };
        let provider_response = match self
            .polymarket_client
            .submit_signed_order(&credentials, &polymarket_payload)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.mark_failed(trade, &error.to_string(), Some(payload.signed_order))
                    .await?;
                return Err(error);
            }
        };

        if provider_success(&provider_response) == Some(false) {
            let message = provider_error_message(&provider_response)
                .unwrap_or_else(|| "Polymarket rejected the order".to_owned());
            self.mark_failed(trade, &message, Some(payload.signed_order))
                .await?;
            return Err(AppError::ExternalApiError(message));
        }

        let mut active = trade.into_active_model();
        active.status = Set(provider_status(&provider_response));
        active.signed_order = Set(Some(payload.signed_order));
        active.provider_response = Set(Some(provider_response.clone()));
        active.provider_order_id = Set(provider_order_id(&provider_response));
        active.error = Set(None);
        active.submitted_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let trade = active.update(&self.db).await?;

        Ok(SubmitSignedTradeResponse {
            provider_response,
            trade: trade_response(trade),
        })
    }

    async fn mark_failed(
        &self,
        trade: trade_intent::Model,
        message: &str,
        signed_order: Option<Value>,
    ) -> Result<trade_intent::Model, AppError> {
        let mut active = trade.into_active_model();
        active.status = Set(TradeIntentStatus::Failed.as_str().to_owned());
        active.signed_order = Set(signed_order);
        active.error = Set(Some(message.to_owned()));
        active.updated_at = Set(Utc::now().into());
        Ok(active.update(&self.db).await?)
    }

    async fn find_owned_trade(
        &self,
        user_id: &str,
        trade_id: &str,
    ) -> Result<trade_intent::Model, AppError> {
        let trade_id = clean_required(trade_id, "trade id is required")?;

        trade_intent::Entity::find_by_id(trade_id)
            .filter(trade_intent::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("trade not found".to_owned()))
    }

    async fn validate_readiness(
        &self,
        user_id: &str,
        provider: SupportedVenue,
        wallet_address: &str,
    ) -> Result<(), AppError> {
        if provider != SupportedVenue::Polymarket {
            return Err(AppError::BadRequest(
                "unsupported trading provider".to_owned(),
            ));
        }

        let wallet_address = normalize_wallet_address(wallet_address)?;
        let user = user::Entity::find_by_id(user_id.to_owned())
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let preferred_provider = user
            .preferred_trading_provider
            .as_deref()
            .and_then(SupportedVenue::from_storage_value)
            .ok_or_else(|| {
                AppError::BadRequest("select a trading provider before trading".to_owned())
            })?;

        if preferred_provider != provider {
            return Err(AppError::BadRequest(
                "selected trading provider does not match this trade".to_owned(),
            ));
        }

        let Some(stored_wallet) = user.primary_wallet_address else {
            return Err(AppError::BadRequest(
                "connect a wallet before trading".to_owned(),
            ));
        };

        if !same_wallet(&stored_wallet, &wallet_address) {
            return Err(AppError::BadRequest(
                "connected wallet does not match this trade".to_owned(),
            ));
        }

        Ok(())
    }

    async fn credentials(
        &self,
        user_id: &str,
        provider: SupportedVenue,
        wallet_address: &str,
    ) -> Result<PolymarketApiCredentials, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let connection = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(user_id))
            .filter(venue_connection::Column::Venue.eq(provider.id()))
            .filter(venue_connection::Column::Enabled.eq(true))
            .filter(venue_connection::Column::Status.eq("active"))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest("sync Polymarket credentials before trading".to_owned())
            })?;

        if !same_wallet(&connection.account_identifier, &wallet_address) {
            return Err(AppError::BadRequest(
                "Polymarket credentials do not match the connected wallet".to_owned(),
            ));
        }

        let config = decrypt_json(&self.credential_encryption_key, &connection.config)?;
        Ok(PolymarketApiCredentials {
            address: wallet_address,
            api_key: string_config(&config, &["apiKey", "api_key", "key"])?,
            secret: string_config(&config, &["secret"])?,
            passphrase: string_config(&config, &["passphrase"])?,
        })
    }
}

fn validate_trade_payload(payload: &CreateTradeIntentRequest) -> Result<(), AppError> {
    clean_required(&payload.market_id, "market id is required")?;
    clean_required(&payload.market_title, "market title is required")?;
    clean_required(&payload.token_id, "token id is required")?;
    clean_required(&payload.outcome, "outcome is required")?;

    if !payload.amount.is_finite() || payload.amount <= 0.0 {
        return Err(AppError::BadRequest(
            "trade amount must be positive".to_owned(),
        ));
    }

    if payload.order_type == TradeOrderType::Limit {
        let Some(price) = payload.price else {
            return Err(AppError::BadRequest("limit price is required".to_owned()));
        };

        if !price.is_finite() || price <= 0.0 || price >= 1.0 {
            return Err(AppError::BadRequest(
                "limit price must be between 0 and 1".to_owned(),
            ));
        }
    }

    Ok(())
}

fn validate_signed_order(order: &Value, trade: &trade_intent::Model) -> Result<(), AppError> {
    let token_id = order
        .get("tokenId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("signed order tokenId is required".to_owned()))?;
    let signer = order
        .get("signer")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("signed order signer is required".to_owned()))?;

    if token_id != trade.token_id {
        return Err(AppError::BadRequest(
            "signed order token does not match this trade".to_owned(),
        ));
    }

    if !same_wallet(signer, &trade.wallet_address) {
        return Err(AppError::BadRequest(
            "signed order signer does not match connected wallet".to_owned(),
        ));
    }

    Ok(())
}

fn string_config(value: &Value, keys: &[&str]) -> Result<String, AppError> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::DatabaseError("Polymarket credentials are incomplete".to_owned()))
}

fn provider_success(value: &Value) -> Option<bool> {
    value.get("success").and_then(Value::as_bool)
}

fn provider_status(value: &Value) -> String {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("submitted")
        .to_ascii_lowercase();

    match status.as_str() {
        "matched" | "filled" => TradeIntentStatus::Filled.as_str().to_owned(),
        "partial" | "partially_filled" => TradeIntentStatus::PartiallyFilled.as_str().to_owned(),
        "rejected" => TradeIntentStatus::Rejected.as_str().to_owned(),
        _ => TradeIntentStatus::Submitted.as_str().to_owned(),
    }
}

fn provider_order_id(value: &Value) -> Option<String> {
    value
        .get("orderID")
        .or_else(|| value.get("order_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn provider_error_message(value: &Value) -> Option<String> {
    value
        .get("errorMsg")
        .or_else(|| value.get("error"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn trade_response(model: trade_intent::Model) -> TradeIntentResponse {
    TradeIntentResponse {
        amount: model.amount,
        automation_id: model.automation_id,
        chain: model.chain,
        chain_id: model.chain_id,
        created_at: model.created_at.to_rfc3339(),
        error: model.error,
        execution_type: model.execution_type,
        id: model.id,
        market_id: model.market_id,
        market_title: model.market_title,
        order_type: model.order_type,
        outcome: model.outcome,
        price: model.price,
        provider: model.provider,
        provider_order_id: model.provider_order_id,
        provider_response: model.provider_response,
        side: model.side,
        status: model.status,
        submitted_at: model.submitted_at.map(|value| value.to_rfc3339()),
        token_id: model.token_id,
        updated_at: model.updated_at.to_rfc3339(),
        wallet_address: model.wallet_address,
    }
}

fn clean_required(value: &str, message: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(message.to_owned()));
    }
    Ok(value.to_owned())
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}
