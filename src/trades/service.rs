use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{Value, json};
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
    providers::{
        polymarket::{
            client::PolymarketSubmissionError,
            credentials::PolymarketApiCredentials,
            dto::{PolymarketExecutionType, PolymarketSignedOrderPayload},
        },
        registry::{ProviderRegistry, ProviderTradingCredentials, ResolvedInstrument},
        types::ProviderId,
    },
    trades::dto::{
        CancelMarketTradesRequest, CancelMultipleTradesRequest, CancelTradesResponse,
        CreateTradeIntentRequest, CreateTradeIntentResponse, ReconcileTradeResponse,
        SubmitSignedTradeRequest, SubmitSignedTradeResponse, TradeIntentResponse,
        TradeIntentStatus, TradeOrderType,
    },
};

#[derive(Clone)]
pub struct TradeService {
    credential_encryption_key: [u8; 32],
    db: Db,
    providers: ProviderRegistry,
}

impl TradeService {
    pub fn new(db: Db, providers: ProviderRegistry, credential_encryption_key: String) -> Self {
        Self {
            credential_encryption_key: parse_encryption_key(&credential_encryption_key)
                .expect("CREDENTIAL_ENCRYPTION_KEY must resolve to 32 bytes"),
            db,
            providers,
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
        let (resolved, token_metadata) = self
            .providers
            .resolve_instrument(
                payload.provider,
                &payload.market_id,
                &payload.token_id,
                &payload.outcome,
            )
            .await?;
        let chain = resolved.chain;
        let now = Utc::now();
        let model = trade_intent::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            automation_id: Set(clean_optional(payload.automation_id)),
            provider: Set(resolved.provider.storage_value().to_owned()),
            chain: Set(chain.storage_value().to_owned()),
            chain_id: Set(chain.id().value() as i64),
            market_id: Set(resolved.market_id),
            market_title: Set(resolved.market_title),
            token_id: Set(resolved.token_id),
            outcome: Set(resolved.outcome),
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
            signed_maker_amount_base: Set(None),
            signed_taker_amount_base: Set(None),
            normalized_amount_base: Set(None),
            normalized_price_numerator: Set(None),
            normalized_price_denominator: Set(None),
            cancellation_requested_at: Set(None),
            cancelled_at: Set(None),
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
        let provider = stored_provider(&trade.provider)?;
        self.validate_readiness(user_id, provider, &trade.wallet_address)
            .await?;
        let (resolved, _) = self
            .providers
            .resolve_instrument(provider, &trade.market_id, &trade.token_id, &trade.outcome)
            .await?;
        validate_resolved_trade(&trade, &resolved)?;
        let credentials = self
            .credentials(user_id, provider, &trade.wallet_address)
            .await?;
        let economics = match &credentials {
            ProviderTradingCredentials::Polymarket(credentials) => {
                validate_signed_order(&payload.signed_order, &trade, credentials)?
            }
        };
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

        let claimed_trade = self
            .claim_submission(user_id, &trade, &payload, &signed_order_hash, &economics)
            .await?;
        let polymarket_payload = PolymarketSignedOrderPayload {
            defer_exec: claimed_trade.defer_exec,
            execution_type: claimed_trade.execution_type.clone(),
            post_only: Some(claimed_trade.post_only),
            signed_order: payload.signed_order,
        };
        let provider_response = match self
            .providers
            .submit_signed_order(provider, &credentials, &polymarket_payload)
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
        if [
            TradeIntentStatus::Filled,
            TradeIntentStatus::Cancelled,
            TradeIntentStatus::Rejected,
            TradeIntentStatus::Failed,
        ]
        .into_iter()
        .any(|status| trade.status == status.as_str())
        {
            return Err(AppError::Conflict(
                "terminal trades cannot be reconciled".to_owned(),
            ));
        }

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
        }

        let Some(provider_order_id) = trade.provider_order_id.clone() else {
            if trade.status != TradeIntentStatus::Submitting.as_str()
                && trade.status != TradeIntentStatus::ReconciliationRequired.as_str()
            {
                return Err(AppError::Conflict(
                    "trade does not require reconciliation".to_owned(),
                ));
            }
            let mut active = <trade_intent::ActiveModel as Default>::default();
            active.status = Set(TradeIntentStatus::ReconciliationRequired
                .as_str()
                .to_owned());
            active.error = Set(Some(
                "No provider order ID is available for REST reconciliation; await the user stream or verify in Polymarket"
                    .to_owned(),
            ));
            active.reconciliation_checked_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            trade_intent::Entity::update_many()
                .set(active)
                .filter(trade_intent::Column::Id.eq(&trade.id))
                .filter(trade_intent::Column::UserId.eq(user_id))
                .exec(&self.db)
                .await?;
            return Ok(ReconcileTradeResponse {
                provider_lookup_available: false,
                resolution: "Provider lookup requires a provider_order_id".to_owned(),
                trade: trade_response(self.find_owned_trade(user_id, trade_id).await?),
            });
        };

        let provider = stored_provider(&trade.provider)?;
        let credentials = self
            .credentials(user_id, provider, &trade.wallet_address)
            .await?;
        let order = self
            .providers
            .get_order(provider, &credentials, &provider_order_id)
            .await?;
        let mut trade_payloads = Vec::new();
        if let Some(trade_ids) = order.get("associate_trades").and_then(Value::as_array) {
            for provider_trade_id in trade_ids.iter().filter_map(Value::as_str) {
                let payload = self
                    .providers
                    .get_trades(provider, &credentials, provider_trade_id)
                    .await?;
                trade_payloads.extend(provider_trade_values(&payload));
            }
        }
        let status = reconciled_status(&order, &trade_payloads);
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(status.clone());
        active.provider_response = Set(Some(json!({
            "reconciliation": {
                "order": order,
                "trades": trade_payloads
            }
        })));
        active.reconciliation_checked_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.error = Set(None);
        if status == TradeIntentStatus::Cancelled.as_str() {
            active.cancelled_at = Set(Some(now.into()));
        }
        trade_intent::Entity::update_many()
            .set(active)
            .filter(trade_intent::Column::Id.eq(&trade.id))
            .filter(trade_intent::Column::UserId.eq(user_id))
            .exec(&self.db)
            .await?;

        Ok(ReconcileTradeResponse {
            provider_lookup_available: true,
            resolution: format!(
                "{} REST reconciliation resolved status to {status}",
                provider.route_value()
            ),
            trade: trade_response(self.find_owned_trade(user_id, trade_id).await?),
        })
    }

    pub async fn cancel_one(
        &self,
        user_id: &str,
        trade_id: &str,
    ) -> Result<CancelTradesResponse, AppError> {
        let trade = self.find_owned_trade(user_id, trade_id).await?;
        if trade.status == TradeIntentStatus::Cancelled.as_str() {
            return Ok(CancelTradesResponse {
                provider_response: json!({
                    "canceled": trade.provider_order_id.clone().into_iter().collect::<Vec<_>>(),
                    "not_canceled": {}
                }),
                trades: vec![trade_response(trade)],
            });
        }
        let order_id = required_provider_order_id(&trade)?;
        let provider = stored_provider(&trade.provider)?;
        let credentials = self.credentials_for_trade(user_id, &trade).await?;
        self.mark_cancellation_requested(user_id, &[trade.id.clone()])
            .await?;
        let provider_response = self
            .providers
            .cancel_order(provider, &credentials, &order_id)
            .await?;
        self.apply_cancellation_confirmation(user_id, provider, &provider_response)
            .await?;
        Ok(CancelTradesResponse {
            provider_response,
            trades: vec![trade_response(
                self.find_owned_trade(user_id, &trade.id).await?,
            )],
        })
    }

    pub async fn cancel_multiple(
        &self,
        user_id: &str,
        payload: CancelMultipleTradesRequest,
    ) -> Result<CancelTradesResponse, AppError> {
        if payload.trade_ids.is_empty() || payload.trade_ids.len() > 1000 {
            return Err(AppError::BadRequest(
                "trade_ids must contain between 1 and 1000 items".to_owned(),
            ));
        }
        let mut trades = Vec::new();
        for trade_id in payload.trade_ids {
            if trades
                .iter()
                .any(|trade: &trade_intent::Model| trade.id == trade_id)
            {
                continue;
            }
            trades.push(self.find_owned_trade(user_id, &trade_id).await?);
        }
        let first = trades
            .first()
            .ok_or_else(|| AppError::BadRequest("trade_ids are required".to_owned()))?;
        let provider = stored_provider(&first.provider)?;
        if trades.iter().any(|trade| trade.provider != first.provider) {
            return Err(AppError::BadRequest(
                "all cancellations in one request must use the same stored provider".to_owned(),
            ));
        }
        let credentials = self.credentials_for_trade(user_id, first).await?;
        let order_ids = trades
            .iter()
            .filter(|trade| trade.status != TradeIntentStatus::Cancelled.as_str())
            .map(required_provider_order_id)
            .collect::<Result<Vec<_>, _>>()?;
        if order_ids.is_empty() {
            return Ok(CancelTradesResponse {
                provider_response: json!({"canceled": [], "not_canceled": {}}),
                trades: trades.into_iter().map(trade_response).collect(),
            });
        }
        let trade_ids = trades
            .iter()
            .map(|trade| trade.id.clone())
            .collect::<Vec<_>>();
        self.mark_cancellation_requested(user_id, &trade_ids)
            .await?;
        let provider_response = self
            .providers
            .cancel_orders(provider, &credentials, &order_ids)
            .await?;
        self.apply_cancellation_confirmation(user_id, provider, &provider_response)
            .await?;
        let refreshed = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::Id.is_in(trade_ids))
            .all(&self.db)
            .await?;
        Ok(CancelTradesResponse {
            provider_response,
            trades: refreshed.into_iter().map(trade_response).collect(),
        })
    }

    pub async fn cancel_all(&self, user_id: &str) -> Result<CancelTradesResponse, AppError> {
        let trades = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::ProviderOrderId.is_not_null())
            .filter(trade_intent::Column::Status.is_in(cancellable_order_statuses()))
            .all(&self.db)
            .await?;
        if trades.is_empty() {
            return Ok(CancelTradesResponse {
                provider_response: json!({"canceled": [], "not_canceled": {}}),
                trades: Vec::new(),
            });
        }

        let groups = group_trades_by_provider(trades)?;
        let aggregate_response = groups.len() > 1;
        let mut provider_responses = serde_json::Map::new();
        let mut single_response = None;
        let mut trade_ids = Vec::new();

        for (provider, provider_trades) in groups {
            let first = provider_trades
                .first()
                .expect("provider trade group is never empty");
            let credentials = self.credentials_for_trade(user_id, first).await?;
            let provider_trade_ids = provider_trades
                .iter()
                .map(|trade| trade.id.clone())
                .collect::<Vec<_>>();
            self.mark_cancellation_requested(user_id, &provider_trade_ids)
                .await?;
            let response = self
                .providers
                .cancel_all_orders(provider, &credentials)
                .await?;
            self.apply_cancellation_confirmation(user_id, provider, &response)
                .await?;

            if aggregate_response {
                provider_responses.insert(provider.api_value().to_owned(), response);
            } else {
                single_response = Some(response);
            }
            trade_ids.extend(provider_trade_ids);
        }

        let provider_response =
            single_response.unwrap_or_else(|| Value::Object(provider_responses));
        Ok(CancelTradesResponse {
            provider_response,
            trades: self.list_models_by_ids(user_id, trade_ids).await?,
        })
    }

    pub async fn cancel_market(
        &self,
        user_id: &str,
        payload: CancelMarketTradesRequest,
    ) -> Result<CancelTradesResponse, AppError> {
        let market_id = clean_required(&payload.market_id, "market_id is required")?;
        let token_id = clean_required(&payload.token_id, "token_id is required")?;
        let provider = payload.provider;
        let trades = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::Provider.eq(provider.storage_value()))
            .filter(trade_intent::Column::MarketId.eq(&market_id))
            .filter(trade_intent::Column::TokenId.eq(&token_id))
            .filter(trade_intent::Column::ProviderOrderId.is_not_null())
            .all(&self.db)
            .await?;
        let Some(first) = trades.first() else {
            return Ok(CancelTradesResponse {
                provider_response: json!({"canceled": [], "not_canceled": {}}),
                trades: Vec::new(),
            });
        };
        let credentials = self.credentials_for_trade(user_id, first).await?;
        let trade_ids = cancellable_trade_ids(&trades);
        self.mark_cancellation_requested(user_id, &trade_ids)
            .await?;
        let provider_response = self
            .providers
            .cancel_market_orders(provider, &credentials, &market_id, &token_id)
            .await?;
        self.apply_cancellation_confirmation(user_id, provider, &provider_response)
            .await?;
        Ok(CancelTradesResponse {
            provider_response,
            trades: self.list_models_by_ids(user_id, trade_ids).await?,
        })
    }

    async fn claim_submission(
        &self,
        user_id: &str,
        trade: &trade_intent::Model,
        payload: &SubmitSignedTradeRequest,
        signed_order_hash: &str,
        economics: &SignedEconomics,
    ) -> Result<trade_intent::Model, AppError> {
        let duplicate = trade_intent::Entity::find()
            .filter(trade_intent::Column::Provider.eq(&trade.provider))
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
        active.signed_maker_amount_base = Set(Some(economics.maker_amount.to_string()));
        active.signed_taker_amount_base = Set(Some(economics.taker_amount.to_string()));
        active.normalized_amount_base = Set(Some(economics.normalized_amount.to_string()));
        active.normalized_price_numerator =
            Set(economics.price_numerator.map(|value| value.to_string()));
        active.normalized_price_denominator =
            Set(economics.price_denominator.map(|value| value.to_string()));
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
        provider: ProviderId,
        wallet_address: &str,
    ) -> Result<(), AppError> {
        self.providers.adapter(provider)?;
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let user = user::Entity::find_by_id(user_id.to_owned())
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let preferred_provider = ProviderId::from_storage(&user.preferred_trading_provider)
            .ok_or_else(|| {
                AppError::BadRequest("select a trading provider before trading".to_owned())
            })?;

        if preferred_provider != provider {
            return Err(AppError::ProviderValidation {
                code: "SELECTED_PROVIDER_MISMATCH",
                message: "selected trading provider does not match this trade".to_owned(),
            });
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

    async fn credentials_for_trade(
        &self,
        user_id: &str,
        trade: &trade_intent::Model,
    ) -> Result<ProviderTradingCredentials, AppError> {
        let provider = stored_provider(&trade.provider)?;
        self.credentials(user_id, provider, &trade.wallet_address)
            .await
    }

    async fn mark_cancellation_requested(
        &self,
        user_id: &str,
        trade_ids: &[String],
    ) -> Result<(), AppError> {
        if trade_ids.is_empty() {
            return Ok(());
        }
        let now = Utc::now();
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(TradeIntentStatus::CancellationRequested.as_str().to_owned());
        active.cancellation_requested_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        trade_intent::Entity::update_many()
            .set(active)
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::Id.is_in(trade_ids.to_vec()))
            .filter(trade_intent::Column::Status.is_in(cancellable_statuses()))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn apply_cancellation_confirmation(
        &self,
        user_id: &str,
        provider: ProviderId,
        provider_response: &Value,
    ) -> Result<(), AppError> {
        let canceled = provider_response
            .get("canceled")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if canceled.is_empty() {
            return Ok(());
        }
        let now = Utc::now();
        let mut active = <trade_intent::ActiveModel as Default>::default();
        active.status = Set(TradeIntentStatus::Cancelled.as_str().to_owned());
        active.cancelled_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.provider_response = Set(Some(provider_response.clone()));
        active.error = Set(None);
        trade_intent::Entity::update_many()
            .set(active)
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::Provider.eq(provider.storage_value()))
            .filter(trade_intent::Column::ProviderOrderId.is_in(canceled))
            .filter(
                trade_intent::Column::Status.eq(TradeIntentStatus::CancellationRequested.as_str()),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list_models_by_ids(
        &self,
        user_id: &str,
        trade_ids: Vec<String>,
    ) -> Result<Vec<TradeIntentResponse>, AppError> {
        if trade_ids.is_empty() {
            return Ok(Vec::new());
        }
        let trades = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .filter(trade_intent::Column::Id.is_in(trade_ids))
            .order_by_desc(trade_intent::Column::UpdatedAt)
            .all(&self.db)
            .await?;
        Ok(trades.into_iter().map(trade_response).collect())
    }

    async fn credentials(
        &self,
        user_id: &str,
        provider: ProviderId,
        wallet_address: &str,
    ) -> Result<ProviderTradingCredentials, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let connection = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(user_id))
            .filter(venue_connection::Column::Provider.eq(provider.storage_value()))
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
        let signature_type = integer_config(&config, "signatureType")?;
        if signature_type != 0 {
            return Err(AppError::BadRequest(
                "Polymarket private beta supports EOA connections only".to_owned(),
            ));
        }
        let funder = normalize_wallet_address(&string_config(&config, &["funder"])?)?;
        if !same_wallet(&funder, &connection.account_identifier)
            || !same_wallet(&funder, &wallet_address)
        {
            return Err(AppError::BadRequest(
                "Polymarket EOA funder, account, and connected wallet must match".to_owned(),
            ));
        }
        Ok(ProviderTradingCredentials::Polymarket(
            PolymarketApiCredentials {
                address: wallet_address,
                funder,
                signature_type,
                api_key: string_config(&config, &["apiKey", "api_key", "key"])?,
                secret: string_config(&config, &["secret"])?,
                passphrase: string_config(&config, &["passphrase"])?,
            },
        ))
    }
}

fn stored_provider(value: &str) -> Result<ProviderId, AppError> {
    ProviderId::from_storage(value)
        .ok_or_else(|| AppError::BadRequest("stored trade provider is invalid".to_owned()))
}

fn validate_resolved_trade(
    trade: &trade_intent::Model,
    resolved: &ResolvedInstrument,
) -> Result<(), AppError> {
    if trade.provider != resolved.provider.storage_value() {
        return Err(AppError::ProviderValidation {
            code: "PROVIDER_MISMATCH",
            message: "resolved provider does not match the stored trade provider".to_owned(),
        });
    }
    if trade.chain != resolved.chain.storage_value()
        || trade.chain_id != resolved.chain.id().value() as i64
    {
        return Err(AppError::ProviderValidation {
            code: "PROVIDER_CHAIN_MISMATCH",
            message: "resolved chain does not match the stored trade chain".to_owned(),
        });
    }
    if trade.market_id != resolved.market_id
        || trade.token_id != resolved.token_id
        || !trade.outcome.eq_ignore_ascii_case(&resolved.outcome)
    {
        return Err(AppError::ProviderValidation {
            code: "PROVIDER_INSTRUMENT_MISMATCH",
            message: "resolved market instrument does not match the stored trade".to_owned(),
        });
    }
    Ok(())
}

fn required_provider_order_id(trade: &trade_intent::Model) -> Result<String, AppError> {
    trade
        .provider_order_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Conflict("trade has no provider order id to cancel".to_owned()))
}

fn cancellable_statuses() -> Vec<String> {
    [
        TradeIntentStatus::Submitted,
        TradeIntentStatus::Matched,
        TradeIntentStatus::Mined,
        TradeIntentStatus::Retrying,
        TradeIntentStatus::PartiallyFilled,
    ]
    .into_iter()
    .map(|status| status.as_str().to_owned())
    .collect()
}

fn cancellable_order_statuses() -> Vec<String> {
    let mut statuses = cancellable_statuses();
    statuses.push(TradeIntentStatus::CancellationRequested.as_str().to_owned());
    statuses
}

fn cancellable_trade_ids(trades: &[trade_intent::Model]) -> Vec<String> {
    let statuses = cancellable_order_statuses();
    trades
        .iter()
        .filter(|trade| statuses.contains(&trade.status))
        .map(|trade| trade.id.clone())
        .collect()
}

fn group_trades_by_provider(
    trades: Vec<trade_intent::Model>,
) -> Result<Vec<(ProviderId, Vec<trade_intent::Model>)>, AppError> {
    let mut groups: Vec<(ProviderId, Vec<trade_intent::Model>)> = Vec::new();
    for trade in trades {
        let provider = stored_provider(&trade.provider)?;
        if let Some((_, provider_trades)) = groups
            .iter_mut()
            .find(|(group_provider, _)| *group_provider == provider)
        {
            provider_trades.push(trade);
        } else {
            groups.push((provider, vec![trade]));
        }
    }
    Ok(groups)
}

fn provider_trade_values(payload: &Value) -> Vec<Value> {
    payload
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .map(|values| values.to_vec())
        .unwrap_or_else(|| {
            payload
                .is_object()
                .then(|| vec![payload.clone()])
                .unwrap_or_default()
        })
}

fn normalized_provider_status(value: &str) -> String {
    let normalized = value.trim().to_ascii_uppercase();
    normalized
        .strip_prefix("TRADE_STATUS_")
        .or_else(|| normalized.strip_prefix("ORDER_STATUS_"))
        .unwrap_or(&normalized)
        .to_owned()
}

fn reconciled_status(order: &Value, trades: &[Value]) -> String {
    let trade_statuses = trades
        .iter()
        .filter_map(|trade| trade.get("status").and_then(Value::as_str))
        .map(normalized_provider_status)
        .collect::<Vec<_>>();
    for (provider_status, status) in [
        ("CONFIRMED", TradeIntentStatus::Filled),
        ("FAILED", TradeIntentStatus::Failed),
        ("RETRYING", TradeIntentStatus::Retrying),
        ("MINED", TradeIntentStatus::Mined),
        ("MATCHED", TradeIntentStatus::Matched),
    ] {
        if trade_statuses.iter().any(|value| value == provider_status) {
            return status.as_str().to_owned();
        }
    }
    let order_status = order
        .get("status")
        .and_then(Value::as_str)
        .map(normalized_provider_status)
        .unwrap_or_default();
    match order_status.as_str() {
        "CANCELED" | "CANCELED_MARKET_RESOLVED" => TradeIntentStatus::Cancelled.as_str().to_owned(),
        "MATCHED" => TradeIntentStatus::Matched.as_str().to_owned(),
        "INVALID" => TradeIntentStatus::Rejected.as_str().to_owned(),
        _ => TradeIntentStatus::Submitted.as_str().to_owned(),
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

    if payload.execution_type == PolymarketExecutionType::Gtd {
        return Err(AppError::BadRequest(
            "GTD orders are not supported during private beta; use LIMIT GTC".to_owned(),
        ));
    }

    match (payload.order_type, payload.execution_type) {
        (TradeOrderType::Market, PolymarketExecutionType::Fok | PolymarketExecutionType::Fak)
        | (TradeOrderType::Limit, PolymarketExecutionType::Gtc) => {}
        (TradeOrderType::Market, _) => {
            return Err(AppError::BadRequest(
                "MARKET orders require FOK or FAK execution".to_owned(),
            ));
        }
        (TradeOrderType::Limit, _) => {
            return Err(AppError::BadRequest(
                "LIMIT orders require GTC execution during private beta".to_owned(),
            ));
        }
    }

    if payload.post_only
        && !matches!(
            payload.execution_type,
            PolymarketExecutionType::Gtc | PolymarketExecutionType::Gtd
        )
    {
        return Err(AppError::BadRequest(
            "postOnly is supported only with GTC or GTD execution".to_owned(),
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

#[derive(Debug, Eq, PartialEq)]
struct SignedEconomics {
    maker_amount: u128,
    taker_amount: u128,
    normalized_amount: u128,
    price_numerator: Option<u128>,
    price_denominator: Option<u128>,
}

fn validate_signed_order(
    order: &Value,
    trade: &trade_intent::Model,
    credentials: &PolymarketApiCredentials,
) -> Result<SignedEconomics, AppError> {
    let token_id = order
        .get("tokenId")
        .and_then(value_to_text)
        .ok_or_else(|| AppError::BadRequest("signed order tokenId is required".to_owned()))?;
    let maker = order
        .get("maker")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("signed order maker is required".to_owned()))?;
    let signer = order
        .get("signer")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("signed order signer is required".to_owned()))?;
    let signature_type = order_integer(order, "signatureType")?;
    let side = signed_order_side(order)?;
    let maker_amount = positive_order_integer(order, "makerAmount")?;
    let taker_amount = positive_order_integer(order, "takerAmount")?;

    if credentials.signature_type != 0 || signature_type != 0 {
        return Err(AppError::BadRequest(
            "Polymarket private beta accepts EOA signed orders only".to_owned(),
        ));
    }
    if token_id != trade.token_id {
        return Err(AppError::BadRequest(
            "signed order token does not match this trade".to_owned(),
        ));
    }
    if !same_wallet(maker, &credentials.funder) || !same_wallet(maker, &credentials.address) {
        return Err(AppError::BadRequest(
            "signed order maker does not match the stored EOA funder and account".to_owned(),
        ));
    }
    if !same_wallet(signer, &credentials.address) || !same_wallet(signer, &trade.wallet_address) {
        return Err(AppError::BadRequest(
            "signed order signer does not match the stored EOA account".to_owned(),
        ));
    }
    if side != trade.side {
        return Err(AppError::BadRequest(
            "signed order side does not match trade intent".to_owned(),
        ));
    }

    let requested_amount = normalized_requested_base(trade.amount, "trade amount")?;
    let normalized_amount = if trade.order_type == TradeOrderType::Limit.as_str() {
        if side == "BUY" {
            taker_amount
        } else {
            maker_amount
        }
    } else {
        maker_amount
    };
    if normalized_amount.abs_diff(requested_amount) > 1 {
        return Err(AppError::BadRequest(
            "signed order normalized amount does not match trade intent".to_owned(),
        ));
    }

    let (price_numerator, price_denominator) = if trade.order_type == TradeOrderType::Limit.as_str()
    {
        let intended_price = normalized_requested_base(
            trade.price.ok_or_else(|| {
                AppError::BadRequest("trade intent limit price is missing".to_owned())
            })?,
            "limit price",
        )?;
        let (numerator, denominator) = if side == "BUY" {
            (maker_amount, taker_amount)
        } else {
            (taker_amount, maker_amount)
        };
        let signed_scaled = numerator
            .checked_mul(1_000_000)
            .ok_or_else(|| AppError::BadRequest("signed order economics overflow".to_owned()))?;
        let intended_scaled = denominator
            .checked_mul(intended_price)
            .ok_or_else(|| AppError::BadRequest("signed order economics overflow".to_owned()))?;
        if signed_scaled.abs_diff(intended_scaled) > denominator {
            return Err(AppError::BadRequest(
                "signed order price ratio does not match trade intent".to_owned(),
            ));
        }
        (Some(numerator), Some(denominator))
    } else {
        (None, None)
    };

    Ok(SignedEconomics {
        maker_amount,
        taker_amount,
        normalized_amount,
        price_numerator,
        price_denominator,
    })
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

fn positive_order_integer(order: &Value, key: &str) -> Result<u128, AppError> {
    let value = order_integer(order, key)?;
    if value == 0 {
        return Err(AppError::BadRequest(format!(
            "signed order {key} must be positive"
        )));
    }
    Ok(value)
}

fn order_integer(order: &Value, key: &str) -> Result<u128, AppError> {
    let value = order
        .get(key)
        .and_then(|value| match value {
            Value::String(value) => value.parse::<u128>().ok(),
            Value::Number(value) => value.as_u64().map(u128::from),
            _ => None,
        })
        .ok_or_else(|| AppError::BadRequest(format!("signed order {key} is invalid")))?;
    Ok(value)
}

fn normalized_requested_base(value: f64, label: &str) -> Result<u128, AppError> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || scaled <= 0.0 || scaled > u64::MAX as f64 {
        return Err(AppError::BadRequest(format!("{label} is out of range")));
    }
    Ok(scaled.round() as u128)
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
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

fn integer_config(value: &Value, key: &str) -> Result<i32, AppError> {
    value
        .get(key)
        .and_then(|value| match value {
            Value::Number(value) => value.as_i64().and_then(|value| i32::try_from(value).ok()),
            Value::String(value) => value.parse().ok(),
            _ => None,
        })
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
        "matched" | "filled" => TradeIntentStatus::Matched.as_str().to_owned(),
        "confirmed" => TradeIntentStatus::Filled.as_str().to_owned(),
        "mined" => TradeIntentStatus::Mined.as_str().to_owned(),
        "retrying" => TradeIntentStatus::Retrying.as_str().to_owned(),
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
        signed_maker_amount_base: model.signed_maker_amount_base,
        signed_taker_amount_base: model.signed_taker_amount_base,
        normalized_amount_base: model.normalized_amount_base,
        normalized_price_numerator: model.normalized_price_numerator,
        normalized_price_denominator: model.normalized_price_denominator,
        status: model.status,
        submission_started_at: model.submission_started_at.map(|value| value.to_rfc3339()),
        submitted_at: model.submitted_at.map(|value| value.to_rfc3339()),
        cancellation_requested_at: model
            .cancellation_requested_at
            .map(|value| value.to_rfc3339()),
        cancelled_at: model.cancelled_at.map(|value| value.to_rfc3339()),
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
        providers::{
            polymarket::{credentials::PolymarketApiCredentials, dto::PolymarketExecutionType},
            registry::ResolvedInstrument,
            types::{Chain, ProviderId},
        },
        trades::dto::{
            CreateTradeIntentRequest, SubmitSignedTradeRequest, TradeIntentStatus, TradeOrderType,
            TradeSide,
        },
    };

    use super::{
        cancellable_order_statuses, group_trades_by_provider, provider_order_id, provider_status,
        signed_order_hash, stored_provider, validate_resolved_trade, validate_signed_order,
        validate_submission_options, validate_trade_payload,
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
            signed_maker_amount_base: None,
            signed_taker_amount_base: None,
            normalized_amount_base: None,
            normalized_price_numerator: None,
            normalized_price_denominator: None,
            cancellation_requested_at: None,
            cancelled_at: None,
        }
    }

    fn credentials() -> PolymarketApiCredentials {
        PolymarketApiCredentials {
            address: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            funder: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            signature_type: 0,
            api_key: "api-key".to_owned(),
            secret: "secret".to_owned(),
            passphrase: "passphrase".to_owned(),
        }
    }

    fn request() -> CreateTradeIntentRequest {
        CreateTradeIntentRequest {
            amount: 10.0,
            automation_id: None,
            defer_exec: false,
            market_id: "market-id".to_owned(),
            market_title: "Market".to_owned(),
            outcome: "YES".to_owned(),
            price: Some(0.5),
            provider: ProviderId::Polymarket,
            side: TradeSide::Buy,
            order_type: TradeOrderType::Limit,
            execution_type: PolymarketExecutionType::Gtc,
            post_only: false,
            token_id: "123456789".to_owned(),
            wallet_address: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
        }
    }

    fn signed_buy_order() -> serde_json::Value {
        json!({
            "tokenId": "123456789",
            "maker": "0x1234567890abcdef1234567890abcdef12345678",
            "signer": "0x1234567890abcdef1234567890abcdef12345678",
            "signatureType": 0,
            "side": "BUY",
            "makerAmount": "5000000",
            "takerAmount": "10000000"
        })
    }

    #[test]
    fn accepts_signed_economics_matching_limit_buy_intent() {
        let economics = validate_signed_order(&signed_buy_order(), &trade(), &credentials())
            .expect("valid signed economics");
        assert_eq!(economics.normalized_amount, 10_000_000);
        assert_eq!(economics.price_numerator, Some(5_000_000));
        assert_eq!(economics.price_denominator, Some(10_000_000));
    }

    #[test]
    fn rejects_non_eoa_or_mismatched_maker_and_signer() {
        let mut signature_type = signed_buy_order();
        signature_type["signatureType"] = json!(1);
        assert!(validate_signed_order(&signature_type, &trade(), &credentials()).is_err());

        let mut maker = signed_buy_order();
        maker["maker"] = json!("0x2234567890abcdef1234567890abcdef12345678");
        assert!(validate_signed_order(&maker, &trade(), &credentials()).is_err());

        let mut signer = signed_buy_order();
        signer["signer"] = json!("0x2234567890abcdef1234567890abcdef12345678");
        assert!(validate_signed_order(&signer, &trade(), &credentials()).is_err());
    }

    #[test]
    fn uses_integer_rounding_for_requested_limit_buy_shares() {
        let mut intent = trade();
        intent.amount = 10.0000004;
        assert!(validate_signed_order(&signed_buy_order(), &intent, &credentials()).is_ok());
        intent.amount = 10.0000016;
        assert!(validate_signed_order(&signed_buy_order(), &intent, &credentials()).is_err());
    }

    #[test]
    fn rejects_signed_order_with_different_side_amount_or_price() {
        let mut side = signed_buy_order();
        side["side"] = json!("SELL");
        assert!(validate_signed_order(&side, &trade(), &credentials()).is_err());

        let mut amount = signed_buy_order();
        amount["takerAmount"] = json!("9000000");
        assert!(validate_signed_order(&amount, &trade(), &credentials()).is_err());

        let mut price = signed_buy_order();
        price["makerAmount"] = json!("4000000");
        assert!(validate_signed_order(&price, &trade(), &credentials()).is_err());
    }

    #[test]
    fn accepts_signed_economics_matching_limit_sell_intent() {
        let mut intent = trade();
        intent.side = "SELL".to_owned();
        intent.amount = 3.0;
        intent.price = Some(0.25);
        let order = json!({
            "tokenId": "123456789",
            "maker": "0x1234567890abcdef1234567890abcdef12345678",
            "signer": "0x1234567890abcdef1234567890abcdef12345678",
            "signatureType": 0,
            "side": 1,
            "makerAmount": "3000000",
            "takerAmount": "750000"
        });

        assert!(validate_signed_order(&order, &intent, &credentials()).is_ok());
    }

    #[test]
    fn enforces_order_type_combinations_and_rejects_gtd() {
        let mut payload = request();
        assert!(validate_trade_payload(&payload).is_ok());

        payload.execution_type = PolymarketExecutionType::Fok;
        assert!(validate_trade_payload(&payload).is_err());

        payload.order_type = TradeOrderType::Market;
        payload.price = None;
        assert!(validate_trade_payload(&payload).is_ok());

        payload.execution_type = PolymarketExecutionType::Gtd;
        assert!(validate_trade_payload(&payload).is_err());

        payload.execution_type = PolymarketExecutionType::Fak;
        payload.post_only = true;
        assert!(validate_trade_payload(&payload).is_err());
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
    fn cancel_all_groups_only_cancellable_stored_provider_orders() {
        let statuses = cancellable_order_statuses();
        assert!(statuses.contains(&TradeIntentStatus::Submitted.as_str().to_owned()));
        assert!(statuses.contains(&TradeIntentStatus::CancellationRequested.as_str().to_owned()));
        assert!(!statuses.contains(&TradeIntentStatus::Filled.as_str().to_owned()));
        assert!(!statuses.contains(&TradeIntentStatus::Cancelled.as_str().to_owned()));

        let mut first = trade();
        first.status = TradeIntentStatus::Submitted.as_str().to_owned();
        first.provider_order_id = Some("order-1".to_owned());
        let mut second = first.clone();
        second.id = "trade-2".to_owned();
        second.provider_order_id = Some("order-2".to_owned());
        let groups = group_trades_by_provider(vec![first, second]).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, ProviderId::Polymarket);
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn existing_order_lifecycle_uses_stored_provider_identity() {
        let persisted = trade();
        assert_eq!(
            stored_provider(&persisted.provider).unwrap(),
            ProviderId::Polymarket
        );
    }

    #[test]
    fn revalidation_rejects_stored_chain_or_instrument_drift() {
        let mut persisted = trade();
        let resolved = ResolvedInstrument {
            chain: Chain::Polygon,
            market_id: persisted.market_id.clone(),
            market_title: persisted.market_title.clone(),
            outcome: persisted.outcome.clone(),
            provider: ProviderId::Polymarket,
            token_id: persisted.token_id.clone(),
        };
        assert!(validate_resolved_trade(&persisted, &resolved).is_ok());
        persisted.chain_id = 1;
        assert!(validate_resolved_trade(&persisted, &resolved).is_err());
    }

    #[test]
    fn maps_provider_status_and_order_id() {
        assert_eq!(provider_status(&json!({"status": "matched"})), "matched");
        assert_eq!(provider_status(&json!({"status": "confirmed"})), "filled");
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
