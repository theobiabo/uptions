use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
        client::{PolymarketClient, PolymarketSubmissionError},
        dto::{PolymarketApiCredentials, PolymarketSignedOrderPayload},
    },
    trades::dto::{
        CreateTradeIntentRequest, CreateTradeIntentResponse, ReconcileTradeResponse,
        SubmitSignedTradeRequest, SubmitSignedTradeResponse, TradeIntentResponse,
        TradeIntentStatus, TradeOrderType,
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
            signed_order_hash: Set(None),
            defer_exec: Set(payload.defer_exec),
            post_only: Set(payload.post_only),
            provider_response: Set(None),
            provider_order_id: Set(None),
            error: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            submitted_at: Set(None),
            submission_started_at: Set(None),
            reconciliation_checked_at: Set(None),
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
        validate_submission_options(&payload, &trade)?;
        validate_signed_order(&payload.signed_order, &trade)?;
        let signed_order_hash = signed_order_hash(&payload.signed_order)?;

        if trade.signed_order_hash.as_deref() == Some(&signed_order_hash) {
            if let Some(provider_response) = trade.provider_response.clone() {
                return Ok(SubmitSignedTradeResponse {
                    provider_response,
                    trade: trade_response(trade),
                });
            }
        } else if trade.signed_order_hash.is_some() {
            return Err(AppError::Conflict(
                "trade is already bound to a different signed order".to_owned(),
            ));
        }

        if trade.status != TradeIntentStatus::PendingSignature.as_str()
            && trade.status != TradeIntentStatus::Failed.as_str()
        {
            return Err(AppError::Conflict(
                "trade submission is already in progress or requires reconciliation".to_owned(),
            ));
        }

        let provider = SupportedVenue::from_storage_value(&trade.provider)
            .ok_or_else(|| AppError::BadRequest("trade provider is invalid".to_owned()))?;
        self.validate_readiness(user_id, provider, &trade.wallet_address)
            .await?;
        let credentials = self
            .credentials(user_id, provider, &trade.wallet_address)
            .await?;
        let claimed_trade = self
            .claim_submission(user_id, &trade, &payload, &signed_order_hash)
            .await?;
        let polymarket_payload = PolymarketSignedOrderPayload {
            defer_exec: claimed_trade.defer_exec,
            execution_type: claimed_trade.execution_type.clone(),
            post_only: Some(claimed_trade.post_only),
            signed_order: payload.signed_order,
        };
        let provider_response = match self
            .polymarket_client
            .submit_signed_order(&credentials, &polymarket_payload)
            .await
        {
            Ok(response) => response,
            Err(PolymarketSubmissionError::Definite(message)) => {
                self.transition_submission(
                    &claimed_trade.id,
                    &signed_order_hash,
                    TradeIntentStatus::Failed,
                    &message,
                )
                .await?;
                return Err(AppError::ExternalApiError(message));
            }
            Err(PolymarketSubmissionError::Ambiguous(message)) => {
                let message = format!(
                    "Polymarket submission outcome is unknown and requires reconciliation: {message}"
                );
                self.transition_submission(
                    &claimed_trade.id,
                    &signed_order_hash,
                    TradeIntentStatus::ReconciliationRequired,
                    &message,
                )
                .await?;
                return Err(AppError::ExternalApiError(message));
            }
        };

        if provider_success(&provider_response) == Some(false) {
            let message = provider_error_message(&provider_response)
                .unwrap_or_else(|| "Polymarket rejected the order".to_owned());
            let trade = self
                .finish_submission(
                    &claimed_trade.id,
                    &signed_order_hash,
                    TradeIntentStatus::Rejected,
                    Some(message.clone()),
                    provider_response.clone(),
                )
                .await?;
            return Err(AppError::ExternalApiError(format!(
                "{message}; trade status is {}",
                trade.status
            )));
        }

        let status = provider_status(&provider_response);
        let trade = self
            .finish_submission_with_status(
                &claimed_trade.id,
                &signed_order_hash,
                status,
                None,
                provider_response.clone(),
            )
            .await?;

        Ok(SubmitSignedTradeResponse {
            provider_response,
            trade: trade_response(trade),
        })
    }

    pub async fn reconcile(
        &self,
        user_id: &str,
        trade_id: &str,
    ) -> Result<ReconcileTradeResponse, AppError> {
        let trade = self.find_owned_trade(user_id, trade_id).await?;
        let now = Utc::now();

        if trade.status == TradeIntentStatus::Submitting.as_str() {
            let stale_before = now - Duration::seconds(30);
            if trade
                .submission_started_at
                .is_some_and(|started_at| started_at > stale_before)
            {
                return Err(AppError::Conflict(
                    "trade submission is still in progress".to_owned(),
                ));
            }

            let mut active = <trade_intent::ActiveModel as Default>::default();
            active.status = Set(TradeIntentStatus::ReconciliationRequired
                .as_str()
                .to_owned());
            active.error = Set(Some(
                "Submission did not complete locally; verify the signed order with Polymarket before any retry"
                    .to_owned(),
            ));
            active.reconciliation_checked_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            trade_intent::Entity::update_many()
                .set(active)
                .filter(trade_intent::Column::Id.eq(&trade.id))
                .filter(trade_intent::Column::UserId.eq(user_id))
                .filter(trade_intent::Column::Status.eq(TradeIntentStatus::Submitting.as_str()))
                .exec(&self.db)
                .await?;
        } else if trade.status == TradeIntentStatus::ReconciliationRequired.as_str() {
            let mut active = <trade_intent::ActiveModel as Default>::default();
            active.reconciliation_checked_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            trade_intent::Entity::update_many()
                .set(active)
                .filter(trade_intent::Column::Id.eq(&trade.id))
                .filter(trade_intent::Column::UserId.eq(user_id))
                .filter(
                    trade_intent::Column::Status
                        .eq(TradeIntentStatus::ReconciliationRequired.as_str()),
                )
                .exec(&self.db)
                .await?;
        } else {
            return Err(AppError::Conflict(
                "trade does not require reconciliation".to_owned(),
            ));
        }

        let trade = self.find_owned_trade(user_id, trade_id).await?;
        Ok(ReconcileTradeResponse {
            provider_lookup_available: false,
            resolution: "No provider order-status lookup is implemented; verify the signed order in Polymarket before retrying or cancelling"
                .to_owned(),
            trade: trade_response(trade),
        })
    }

    async fn claim_submission(
        &self,
        user_id: &str,
        trade: &trade_intent::Model,
        payload: &SubmitSignedTradeRequest,
        signed_order_hash: &str,
    ) -> Result<trade_intent::Model, AppError> {
        let duplicate = trade_intent::Entity::find()
            .filter(trade_intent::Column::SignedOrderHash.eq(signed_order_hash))
            .filter(trade_intent::Column::Id.ne(&trade.id))
            .one(&self.db)
            .await?;
        if duplicate.is_some() {
            return Err(AppError::Conflict(
                "signed order is already bound to another trade".to_owned(),
            ));
        }

        let now = Utc::now();
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(TradeIntentStatus::Submitting.as_str().to_owned());
        active.signed_order = Set(Some(payload.signed_order.clone()));
        active.signed_order_hash = Set(Some(signed_order_hash.to_owned()));
        active.error = Set(None);
        active.provider_response = Set(None);
        active.provider_order_id = Set(None);
        active.submission_started_at = Set(Some(now.into()));
        active.reconciliation_checked_at = Set(None);
        active.updated_at = Set(now.into());
        let result = trade_intent::Entity::update_many()
            .set(active)
            .filter(trade_intent::Column::Id.eq(&trade.id))
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(
                Condition::any()
                    .add(
                        trade_intent::Column::Status
                            .eq(TradeIntentStatus::PendingSignature.as_str()),
                    )
                    .add(trade_intent::Column::Status.eq(TradeIntentStatus::Failed.as_str())),
            )
            .filter(
                Condition::any()
                    .add(trade_intent::Column::SignedOrderHash.is_null())
                    .add(trade_intent::Column::SignedOrderHash.eq(signed_order_hash)),
            )
            .exec(&self.db)
            .await
            .map_err(submission_claim_error)?;

        if result.rows_affected != 1 {
            return Err(AppError::Conflict(
                "trade submission was claimed by another request".to_owned(),
            ));
        }

        self.find_owned_trade(user_id, &trade.id).await
    }

    async fn transition_submission(
        &self,
        trade_id: &str,
        signed_order_hash: &str,
        status: TradeIntentStatus,
        message: &str,
    ) -> Result<trade_intent::Model, AppError> {
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(status.as_str().to_owned());
        active.error = Set(Some(message.to_owned()));
        active.updated_at = Set(Utc::now().into());
        self.update_claimed_trade(trade_id, signed_order_hash, active)
            .await
    }

    async fn finish_submission(
        &self,
        trade_id: &str,
        signed_order_hash: &str,
        status: TradeIntentStatus,
        error: Option<String>,
        provider_response: Value,
    ) -> Result<trade_intent::Model, AppError> {
        self.finish_submission_with_status(
            trade_id,
            signed_order_hash,
            status.as_str().to_owned(),
            error,
            provider_response,
        )
        .await
    }

    async fn finish_submission_with_status(
        &self,
        trade_id: &str,
        signed_order_hash: &str,
        status: String,
        error: Option<String>,
        provider_response: Value,
    ) -> Result<trade_intent::Model, AppError> {
        let now = Utc::now();
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(status);
        active.provider_order_id = Set(provider_order_id(&provider_response));
        active.provider_response = Set(Some(provider_response));
        active.error = Set(error);
        active.submitted_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        self.update_claimed_trade(trade_id, signed_order_hash, active)
            .await
    }

    async fn update_claimed_trade(
        &self,
        trade_id: &str,
        signed_order_hash: &str,
        active: trade_intent::ActiveModel,
    ) -> Result<trade_intent::Model, AppError> {
        let result = trade_intent::Entity::update_many()
            .set(active)
            .filter(trade_intent::Column::Id.eq(trade_id))
            .filter(trade_intent::Column::SignedOrderHash.eq(signed_order_hash))
            .filter(trade_intent::Column::Status.eq(TradeIntentStatus::Submitting.as_str()))
            .exec(&self.db)
            .await?;

        if result.rows_affected != 1 {
            return Err(AppError::Conflict(
                "trade submission state changed and requires reconciliation".to_owned(),
            ));
        }

        trade_intent::Entity::find_by_id(trade_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("trade not found".to_owned()))
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

fn validate_submission_options(
    payload: &SubmitSignedTradeRequest,
    trade: &trade_intent::Model,
) -> Result<(), AppError> {
    if payload.execution_type.as_str() != trade.execution_type {
        return Err(AppError::BadRequest(
            "submission execution type does not match trade intent".to_owned(),
        ));
    }
    if payload.defer_exec != trade.defer_exec {
        return Err(AppError::BadRequest(
            "submission deferExec does not match trade intent".to_owned(),
        ));
    }
    if payload.post_only.unwrap_or(false) != trade.post_only {
        return Err(AppError::BadRequest(
            "submission postOnly does not match trade intent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_signed_order(order: &Value, trade: &trade_intent::Model) -> Result<(), AppError> {
    let token_id = order
        .get("tokenId")
        .and_then(value_to_text)
        .ok_or_else(|| AppError::BadRequest("signed order tokenId is required".to_owned()))?;
    let signer = order
        .get("signer")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("signed order signer is required".to_owned()))?;
    let side = signed_order_side(order)?;
    let maker_amount = positive_order_number(order, "makerAmount")?;
    let taker_amount = positive_order_number(order, "takerAmount")?;

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
    if side != trade.side {
        return Err(AppError::BadRequest(
            "signed order side does not match trade intent".to_owned(),
        ));
    }

    let intended_base_amount = trade.amount * 1_000_000.0;
    let signed_base_amount = maker_amount;
    if (signed_base_amount - intended_base_amount).abs() > 1.0 {
        return Err(AppError::BadRequest(
            "signed order amount does not match trade intent".to_owned(),
        ));
    }

    if trade.order_type == TradeOrderType::Limit.as_str() {
        let intended_price = trade.price.ok_or_else(|| {
            AppError::BadRequest("trade intent limit price is missing".to_owned())
        })?;
        let signed_price = if side == "BUY" {
            maker_amount / taker_amount
        } else {
            taker_amount / maker_amount
        };
        if (signed_price - intended_price).abs() > 0.000001 {
            return Err(AppError::BadRequest(
                "signed order price does not match trade intent".to_owned(),
            ));
        }
    }

    Ok(())
}

fn signed_order_side(order: &Value) -> Result<&'static str, AppError> {
    match order.get("side") {
        Some(Value::Number(value)) if value.as_u64() == Some(0) => Ok("BUY"),
        Some(Value::Number(value)) if value.as_u64() == Some(1) => Ok("SELL"),
        Some(Value::String(value)) if value.eq_ignore_ascii_case("BUY") => Ok("BUY"),
        Some(Value::String(value)) if value.eq_ignore_ascii_case("SELL") => Ok("SELL"),
        _ => Err(AppError::BadRequest(
            "signed order side is invalid".to_owned(),
        )),
    }
}

fn positive_order_number(order: &Value, key: &str) -> Result<f64, AppError> {
    let value = order
        .get(key)
        .and_then(value_to_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| AppError::BadRequest(format!("signed order {key} is invalid")))?;
    Ok(value)
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::String(value) => value.parse().ok(),
        Value::Number(value) => value.as_f64(),
        _ => None,
    }
}

fn signed_order_hash(order: &Value) -> Result<String, AppError> {
    let encoded = serde_json::to_vec(order)
        .map_err(|error| AppError::BadRequest(format!("signed order is invalid: {error}")))?;
    let digest = Sha256::digest(encoded);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn submission_claim_error(error: sea_orm::DbErr) -> AppError {
    if error.to_string().to_ascii_lowercase().contains("unique") {
        AppError::Conflict("signed order is already bound to another trade".to_owned())
    } else {
        AppError::DatabaseError(error.to_string())
    }
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
        defer_exec: model.defer_exec,
        error: model.error,
        execution_type: model.execution_type,
        id: model.id,
        market_id: model.market_id,
        market_title: model.market_title,
        order_type: model.order_type,
        outcome: model.outcome,
        post_only: model.post_only,
        price: model.price,
        provider: model.provider,
        provider_order_id: model.provider_order_id,
        provider_response: model.provider_response,
        reconciliation_checked_at: model
            .reconciliation_checked_at
            .map(|value| value.to_rfc3339()),
        side: model.side,
        signed_order_hash: model.signed_order_hash,
        status: model.status,
        submission_started_at: model.submission_started_at.map(|value| value.to_rfc3339()),
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use crate::{
        entities::trade_intent,
        trades::dto::{PolymarketExecutionType, SubmitSignedTradeRequest},
    };

    use super::{
        provider_order_id, provider_status, signed_order_hash, validate_signed_order,
        validate_submission_options,
    };

    fn trade() -> trade_intent::Model {
        let now = Utc::now().into();
        trade_intent::Model {
            id: "trade-id".to_owned(),
            user_id: "user-id".to_owned(),
            automation_id: None,
            provider: "POLYMARKET".to_owned(),
            chain: "POLYGON".to_owned(),
            chain_id: 137,
            market_id: "market-id".to_owned(),
            market_title: "Market".to_owned(),
            token_id: "123456789".to_owned(),
            outcome: "YES".to_owned(),
            side: "BUY".to_owned(),
            order_type: "LIMIT".to_owned(),
            execution_type: "GTC".to_owned(),
            amount: 10.0,
            price: Some(0.5),
            wallet_address: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            status: "pending_signature".to_owned(),
            signed_order: None,
            signed_order_hash: None,
            defer_exec: false,
            post_only: true,
            provider_response: None,
            provider_order_id: None,
            error: None,
            created_at: now,
            updated_at: now,
            submitted_at: None,
            submission_started_at: None,
            reconciliation_checked_at: None,
        }
    }

    fn signed_buy_order() -> serde_json::Value {
        json!({
            "tokenId": "123456789",
            "signer": "0x1234567890abcdef1234567890abcdef12345678",
            "side": "BUY",
            "makerAmount": "10000000",
            "takerAmount": "20000000"
        })
    }

    #[test]
    fn accepts_signed_economics_matching_limit_buy_intent() {
        assert!(validate_signed_order(&signed_buy_order(), &trade()).is_ok());
    }

    #[test]
    fn rejects_signed_order_with_different_side_amount_or_price() {
        let mut side = signed_buy_order();
        side["side"] = json!("SELL");
        assert!(validate_signed_order(&side, &trade()).is_err());

        let mut amount = signed_buy_order();
        amount["makerAmount"] = json!("9000000");
        assert!(validate_signed_order(&amount, &trade()).is_err());

        let mut price = signed_buy_order();
        price["takerAmount"] = json!("25000000");
        assert!(validate_signed_order(&price, &trade()).is_err());
    }

    #[test]
    fn accepts_signed_economics_matching_limit_sell_intent() {
        let mut intent = trade();
        intent.side = "SELL".to_owned();
        intent.amount = 3.0;
        intent.price = Some(0.25);
        let order = json!({
            "tokenId": "123456789",
            "signer": "0x1234567890abcdef1234567890abcdef12345678",
            "side": 1,
            "makerAmount": "3000000",
            "takerAmount": "750000"
        });

        assert!(validate_signed_order(&order, &intent).is_ok());
    }

    #[test]
    fn rejects_execution_options_different_from_intent() {
        let intent = trade();
        let mut payload = SubmitSignedTradeRequest {
            defer_exec: false,
            execution_type: PolymarketExecutionType::Gtc,
            post_only: Some(true),
            signed_order: signed_buy_order(),
        };
        assert!(validate_submission_options(&payload, &intent).is_ok());

        payload.defer_exec = true;
        assert!(validate_submission_options(&payload, &intent).is_err());
        payload.defer_exec = false;
        payload.post_only = Some(false);
        assert!(validate_submission_options(&payload, &intent).is_err());
        payload.post_only = Some(true);
        payload.execution_type = PolymarketExecutionType::Fok;
        assert!(validate_submission_options(&payload, &intent).is_err());
    }

    #[test]
    fn signed_order_hash_is_stable_for_object_key_order() {
        let first = json!({"a": 1, "b": {"x": 2, "y": 3}});
        let second = json!({"b": {"y": 3, "x": 2}, "a": 1});

        assert_eq!(
            signed_order_hash(&first).unwrap(),
            signed_order_hash(&second).unwrap()
        );
    }

    #[test]
    fn maps_provider_status_and_order_id() {
        assert_eq!(provider_status(&json!({"status": "matched"})), "filled");
        assert_eq!(
            provider_status(&json!({"status": "partially_filled"})),
            "partially_filled"
        );
        assert_eq!(provider_status(&json!({"status": "live"})), "submitted");
        assert_eq!(
            provider_order_id(&json!({"orderID": "provider-id"})),
            Some("provider-id".to_owned())
        );
    }
}
