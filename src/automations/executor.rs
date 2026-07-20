use std::collections::HashMap;

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde_json::{Value, json};
use tokio::time::{self, Duration as TokioDuration};
use uuid::Uuid;

use crate::{
    automations::{
        dto::{AutomationStepKind, WorkflowActionType, WorkflowPayload, WorkflowStepPayload},
        service::AutomationService,
    },
    db::Db,
    entities::{automation, automation_observation, automation_run, user},
    error::AppError,
    markets::types::MarketResponse,
    providers::{registry::ProviderRegistry, types::ProviderId},
};

const AUTOMATION_INTERVAL_SECONDS: u64 = 30;
const AUTOMATION_COOLDOWN_SECONDS: i64 = 300;

#[derive(Clone)]
pub struct AutomationExecutor {
    automation_service: AutomationService,
    db: Db,
    providers: ProviderRegistry,
}

struct TriggerEvaluation {
    matched: bool,
    snapshot: Value,
}

impl AutomationExecutor {
    pub fn new(db: Db, automation_service: AutomationService, providers: ProviderRegistry) -> Self {
        Self {
            automation_service,
            db,
            providers,
        }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            let mut interval =
                time::interval(TokioDuration::from_secs(AUTOMATION_INTERVAL_SECONDS));

            loop {
                interval.tick().await;

                if let Err(error) = self.tick().await {
                    tracing::warn!(error = %error, "automation executor tick failed");
                }
            }
        });
    }

    async fn tick(&self) -> Result<(), AppError> {
        let automations = automation::Entity::find()
            .filter(automation::Column::Status.eq("active"))
            .order_by_asc(automation::Column::UpdatedAt)
            .all(&self.db)
            .await?;

        for automation in automations {
            if let Err(error) = self.evaluate_automation(automation).await {
                tracing::warn!(error = %error, "automation evaluation failed");
            }
        }

        Ok(())
    }

    async fn evaluate_automation(&self, automation: automation::Model) -> Result<(), AppError> {
        let provider = self.ensure_runtime_provider_alignment(&automation).await?;
        let workflow = serde_json::from_value::<WorkflowPayload>(automation.workflow.clone())
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let ordered_steps = ordered_workflow_steps(&workflow)?;
        let trigger = ordered_steps
            .first()
            .ok_or_else(|| AppError::BadRequest("workflow trigger is missing".to_owned()))?;
        let market = self.fetch_market(provider, &automation).await?;
        let trigger_evaluation = self
            .evaluate_trigger(&automation.id, trigger, &market)
            .await?;
        let conditions = ordered_steps
            .iter()
            .copied()
            .filter(|step| step.kind == AutomationStepKind::Condition)
            .collect::<Vec<_>>();
        let actions = ordered_steps
            .iter()
            .copied()
            .filter(|step| step.kind == AutomationStepKind::Action)
            .collect::<Vec<_>>();
        let conditions_matched = conditions
            .iter()
            .all(|step| condition_matches(step, Some(&market)));
        let trigger_snapshot = json!({
            "matched": trigger_evaluation.matched,
            "step": trigger,
            "observation": trigger_evaluation.snapshot,
            "market_id": automation.market_id,
            "market_title": automation.market_title
        });
        let condition_snapshot = json!({
            "matched": conditions_matched,
            "steps": conditions
        });

        if !trigger_evaluation.matched || !conditions_matched || actions.is_empty() {
            self.create_run(
                &automation,
                "skipped",
                trigger_snapshot,
                condition_snapshot,
                json!({ "actions": actions }),
                Some(json!({ "reason": "trigger or condition did not match" })),
                None,
            )
            .await?;
            return Ok(());
        }

        if self.is_in_cooldown(&automation.id).await? {
            self.create_run(
                &automation,
                "skipped",
                trigger_snapshot,
                condition_snapshot,
                json!({ "actions": actions }),
                Some(json!({ "reason": "action cooldown is active" })),
                None,
            )
            .await?;
            return Ok(());
        }

        let run_id = Uuid::new_v4().to_string();
        let mut results = Vec::new();
        let mut status = "completed";

        for action in actions {
            let result = self
                .execute_action(&automation, &run_id, action, Some(&market))
                .await?;

            if result
                .get("approval_only")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                status = "approval_notification_sent";
            }

            results.push(result);
        }

        self.insert_run(
            &run_id,
            &automation,
            status,
            trigger_snapshot,
            condition_snapshot,
            json!({ "results": results }),
            Some(json!({ "results": results })),
            None,
        )
        .await
    }

    async fn evaluate_trigger(
        &self,
        automation_id: &str,
        step: &WorkflowStepPayload,
        market: &MarketResponse,
    ) -> Result<TriggerEvaluation, AppError> {
        let previous =
            automation_observation::Entity::find_by_id((automation_id.to_owned(), step.id.clone()))
                .one(&self.db)
                .await?;
        let now = Utc::now();

        match step.action {
            WorkflowActionType::TriggerPriceMoves => {
                let outcome = step
                    .params
                    .get("outcome")
                    .and_then(Value::as_str)
                    .unwrap_or("YES");
                let current = market_price(Some(market), outcome).ok_or_else(|| {
                    AppError::ExternalApiError("provider market price is unavailable".to_owned())
                })?;
                let previous_value = previous.as_ref().and_then(|value| value.value);
                let matched =
                    previous_value.is_some_and(|value| (current - value).abs() > f64::EPSILON);
                self.save_observation(automation_id, &step.id, Some(current), now)
                    .await?;

                Ok(TriggerEvaluation {
                    matched,
                    snapshot: json!({
                        "metric": "outcome_price",
                        "outcome": outcome,
                        "previous": previous_value,
                        "current": current,
                        "baseline_established": previous_value.is_some()
                    }),
                })
            }
            WorkflowActionType::TriggerVolumeMoves => {
                let current = market.volume.ok_or_else(|| {
                    AppError::ExternalApiError("provider market volume is unavailable".to_owned())
                })?;
                let previous_value = previous.as_ref().and_then(|value| value.value);
                let change_percent = previous_value.and_then(|value| {
                    (value.abs() > f64::EPSILON)
                        .then_some(((current - value).abs() / value.abs()) * 100.0)
                });
                let minimum_change = number_param(&step.params, "minimum_change_percent")
                    .ok_or_else(|| {
                        AppError::BadRequest("minimum_change_percent is required".to_owned())
                    })?;
                let matched = change_percent.is_some_and(|value| value >= minimum_change);
                self.save_observation(automation_id, &step.id, Some(current), now)
                    .await?;

                Ok(TriggerEvaluation {
                    matched,
                    snapshot: json!({
                        "metric": "market_volume",
                        "previous": previous_value,
                        "current": current,
                        "change_percent": change_percent,
                        "minimum_change_percent": minimum_change,
                        "baseline_established": previous_value.is_some()
                    }),
                })
            }
            WorkflowActionType::TriggerTimeCheck => {
                let interval = step
                    .params
                    .get("interval")
                    .and_then(Value::as_str)
                    .and_then(schedule_seconds)
                    .ok_or_else(|| AppError::BadRequest("interval is invalid".to_owned()))?;
                let previous_at = previous
                    .as_ref()
                    .map(|value| value.observed_at.with_timezone(&Utc));
                let matched = previous_at.is_some_and(|value| {
                    now.signed_duration_since(value).num_seconds() >= interval
                });

                if previous.is_none() || matched {
                    self.save_observation(automation_id, &step.id, None, now)
                        .await?;
                }

                Ok(TriggerEvaluation {
                    matched,
                    snapshot: json!({
                        "metric": "schedule",
                        "interval_seconds": interval,
                        "previous_check_at": previous_at.map(|value| value.to_rfc3339()),
                        "checked_at": now.to_rfc3339(),
                        "baseline_established": previous_at.is_some()
                    }),
                })
            }
            _ => Err(AppError::BadRequest(
                "workflow must begin with a supported trigger".to_owned(),
            )),
        }
    }

    async fn save_observation(
        &self,
        automation_id: &str,
        step_id: &str,
        value: Option<f64>,
        observed_at: chrono::DateTime<Utc>,
    ) -> Result<(), AppError> {
        let existing = automation_observation::Entity::find_by_id((
            automation_id.to_owned(),
            step_id.to_owned(),
        ))
        .one(&self.db)
        .await?;

        if let Some(existing) = existing {
            let mut active = existing.into_active_model();
            active.value = Set(value);
            active.observed_at = Set(observed_at.into());
            active.update(&self.db).await?;
        } else {
            automation_observation::ActiveModel {
                automation_id: Set(automation_id.to_owned()),
                step_id: Set(step_id.to_owned()),
                value: Set(value),
                observed_at: Set(observed_at.into()),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    async fn execute_action(
        &self,
        automation: &automation::Model,
        run_id: &str,
        action: &WorkflowStepPayload,
        market: Option<&MarketResponse>,
    ) -> Result<Value, AppError> {
        match action.action {
            WorkflowActionType::SendMessage => {
                self.send_message_action(automation, run_id, action).await
            }
            WorkflowActionType::Buy | WorkflowActionType::Sell => {
                self.trade_approval_action(automation, run_id, action, market)
                    .await
            }
            _ => Err(AppError::BadRequest(
                "workflow action is not supported".to_owned(),
            )),
        }
    }

    async fn send_message_action(
        &self,
        automation: &automation::Model,
        run_id: &str,
        action: &WorkflowStepPayload,
    ) -> Result<Value, AppError> {
        let message = action
            .params
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Automation condition matched.");
        let title = format!("Automation triggered: {}", automation.title);

        self.automation_service
            .create_alert(
                &automation.user_id,
                Some(automation.id.clone()),
                &title,
                message,
                "success",
                json!({
                    "type": "automation_message",
                    "automation_id": automation.id,
                    "run_id": run_id,
                    "step_id": action.id
                }),
            )
            .await?;

        Ok(json!({ "action": "send_message", "completed": true }))
    }

    async fn trade_approval_action(
        &self,
        automation: &automation::Model,
        run_id: &str,
        action: &WorkflowStepPayload,
        market: Option<&MarketResponse>,
    ) -> Result<Value, AppError> {
        let side = match action.action {
            WorkflowActionType::Buy => "BUY",
            WorkflowActionType::Sell => "SELL",
            _ => return Err(AppError::BadRequest("trade side is invalid".to_owned())),
        };
        let outcome = action
            .params
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::BadRequest("outcome is required".to_owned()))?;
        let limit_price = number_param(&action.params, "limit_price")
            .ok_or_else(|| AppError::BadRequest("limit_price is required".to_owned()))?;
        let (quantity, quantity_label, usdc_amount, shares) = match action.action {
            WorkflowActionType::Buy => {
                let quantity = number_param(&action.params, "usdc_amount")
                    .ok_or_else(|| AppError::BadRequest("usdc_amount is required".to_owned()))?;
                (quantity, "USDC", Some(quantity), None)
            }
            WorkflowActionType::Sell => {
                let quantity = number_param(&action.params, "shares")
                    .ok_or_else(|| AppError::BadRequest("shares is required".to_owned()))?;
                (quantity, "shares", None, Some(quantity))
            }
            _ => unreachable!(),
        };
        let token_id = market_token_id(market, outcome).ok_or_else(|| {
            AppError::ExternalApiError("Polymarket outcome token is unavailable".to_owned())
        })?;
        let message = format!(
            "Approval notification only: review a {side} limit order for {quantity} {quantity_label} of {outcome} at {limit_price} on {}. No trade was executed.",
            automation.market_title.as_deref().unwrap_or("this market")
        );

        self.automation_service
            .create_alert(
                &automation.user_id,
                Some(automation.id.clone()),
                "Trade approval notification",
                &message,
                "info",
                json!({
                    "type": "automation_trade_approval_notification",
                    "automation_id": automation.id,
                    "run_id": run_id,
                    "market_id": automation.market_id,
                    "market_title": automation.market_title,
                    "token_id": token_id,
                    "side": side,
                    "outcome": outcome,
                    "usdc_amount": usdc_amount,
                    "shares": shares,
                    "order_type": "LIMIT",
                    "limit_price": limit_price,
                    "approval_only": true,
                    "execution_status": "not_executed",
                    "step_id": action.id
                }),
            )
            .await?;

        Ok(json!({
            "action": "trade_approval_notification",
            "approval_only": true,
            "execution_status": "not_executed",
            "side": side,
            "outcome": outcome,
            "usdc_amount": usdc_amount,
            "shares": shares,
            "order_type": "LIMIT",
            "limit_price": limit_price,
            "token_id": token_id
        }))
    }

    async fn fetch_market(
        &self,
        provider: ProviderId,
        automation: &automation::Model,
    ) -> Result<MarketResponse, AppError> {
        let market_id = automation
            .market_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("automation market id is missing".to_owned()))?;
        let resolved = self.providers.resolve_market(provider, market_id).await?;
        if automation.chain != resolved.chain.storage_value()
            || automation.chain_id != resolved.chain.id().value() as i64
        {
            return Err(AppError::ProviderValidation {
                code: "AUTOMATION_CHAIN_MISMATCH",
                message: "automation chain does not match the provider market".to_owned(),
            });
        }
        Ok(resolved.market)
    }

    async fn ensure_runtime_provider_alignment(
        &self,
        automation: &automation::Model,
    ) -> Result<ProviderId, AppError> {
        let provider = ProviderId::from_storage(&automation.provider).ok_or_else(|| {
            AppError::BadRequest("stored automation provider is invalid".to_owned())
        })?;
        let user = user::Entity::find_by_id(&automation.user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("automation user not found".to_owned()))?;
        let selected = ProviderId::from_storage(&user.preferred_trading_provider);
        if selected != Some(provider) {
            let now = Utc::now();
            let mut active = automation.clone().into_active_model();
            active.status = Set("paused".to_owned());
            active.last_run_status = Set(Some("action_required_provider_changed".to_owned()));
            active.updated_at = Set(now.into());
            active.update(&self.db).await?;
            self.automation_service
                .create_alert(
                    &automation.user_id,
                    Some(automation.id.clone()),
                    "Automation paused",
                    "This automation uses a different provider than your selected provider. Review it before resuming.",
                    "warning",
                    json!({
                        "type": "automation_provider_action_required",
                        "automation_id": automation.id,
                        "automation_provider": provider,
                        "selected_provider": selected
                    }),
                )
                .await?;
            return Err(AppError::Conflict(
                "automation paused because its provider no longer matches the selected provider"
                    .to_owned(),
            ));
        }
        Ok(provider)
    }

    async fn is_in_cooldown(&self, automation_id: &str) -> Result<bool, AppError> {
        let since = (Utc::now() - Duration::seconds(AUTOMATION_COOLDOWN_SECONDS)).fixed_offset();
        let run = automation_run::Entity::find()
            .filter(automation_run::Column::AutomationId.eq(automation_id))
            .filter(
                automation_run::Column::Status.is_in(["completed", "approval_notification_sent"]),
            )
            .filter(automation_run::Column::CreatedAt.gt(since))
            .one(&self.db)
            .await?;

        Ok(run.is_some())
    }

    async fn create_run(
        &self,
        automation: &automation::Model,
        status: &str,
        trigger_snapshot: Value,
        condition_snapshot: Value,
        action_snapshot: Value,
        result: Option<Value>,
        error: Option<String>,
    ) -> Result<(), AppError> {
        let run_id = Uuid::new_v4().to_string();

        self.insert_run(
            &run_id,
            automation,
            status,
            trigger_snapshot,
            condition_snapshot,
            action_snapshot,
            result,
            error,
        )
        .await
    }

    async fn insert_run(
        &self,
        run_id: &str,
        automation: &automation::Model,
        status: &str,
        trigger_snapshot: Value,
        condition_snapshot: Value,
        action_snapshot: Value,
        result: Option<Value>,
        error: Option<String>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        automation_run::ActiveModel {
            id: Set(run_id.to_owned()),
            user_id: Set(automation.user_id.clone()),
            automation_id: Set(automation.id.clone()),
            status: Set(status.to_owned()),
            trigger_snapshot: Set(trigger_snapshot),
            condition_snapshot: Set(condition_snapshot),
            action_snapshot: Set(action_snapshot),
            result: Set(result),
            error: Set(error),
            created_at: Set(now.into()),
            completed_at: Set(Some(now.into())),
        }
        .insert(&self.db)
        .await?;

        let mut active = automation.clone().into_active_model();
        active.last_run_status = Set(Some(status.to_owned()));
        active.last_run_at = Set(Some(now.into()));
        active.update(&self.db).await?;

        Ok(())
    }
}

fn ordered_workflow_steps(
    workflow: &WorkflowPayload,
) -> Result<Vec<&WorkflowStepPayload>, AppError> {
    let steps = workflow
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<HashMap<_, _>>();
    let mut incoming = HashMap::<&str, usize>::new();
    let mut outgoing = HashMap::<&str, &str>::new();

    for step in &workflow.steps {
        incoming.insert(step.id.as_str(), 0);
    }

    for connection in &workflow.connections {
        let count = incoming.entry(connection.to.as_str()).or_default();
        *count += 1;

        if *count > 1
            || outgoing
                .insert(connection.from.as_str(), connection.to.as_str())
                .is_some()
        {
            return Err(AppError::BadRequest(
                "workflow must be one linear path without branches".to_owned(),
            ));
        }
    }

    let roots = workflow
        .steps
        .iter()
        .filter(|step| incoming.get(step.id.as_str()) == Some(&0))
        .collect::<Vec<_>>();

    if roots.len() != 1 || workflow.connections.len() + 1 != workflow.steps.len() {
        return Err(AppError::BadRequest(
            "workflow must be one connected linear path".to_owned(),
        ));
    }

    let mut ordered = Vec::with_capacity(workflow.steps.len());
    let mut current = roots[0].id.as_str();

    loop {
        let step = steps.get(current).copied().ok_or_else(|| {
            AppError::BadRequest("workflow connection references a missing step".to_owned())
        })?;
        ordered.push(step);

        let Some(next) = outgoing.get(current) else {
            break;
        };
        current = *next;
    }

    if ordered.len() != workflow.steps.len()
        || ordered.first().map(|step| step.kind) != Some(AutomationStepKind::Trigger)
        || ordered.last().map(|step| step.kind) != Some(AutomationStepKind::Action)
    {
        return Err(AppError::BadRequest(
            "workflow must be one linear path from a trigger to an action".to_owned(),
        ));
    }

    Ok(ordered)
}

fn condition_matches(step: &WorkflowStepPayload, market: Option<&MarketResponse>) -> bool {
    match step.action {
        WorkflowActionType::ConditionOutcomePriceAbove => {
            compare_price(step, market, |current, target| current > target)
        }
        WorkflowActionType::ConditionOutcomePriceBelow => {
            compare_price(step, market, |current, target| current < target)
        }
        WorkflowActionType::ConditionVolumeAbove => market
            .and_then(|market| market.volume)
            .zip(number_param(&step.params, "volume"))
            .is_some_and(|(current, target)| current > target),
        _ => false,
    }
}

fn compare_price(
    step: &WorkflowStepPayload,
    market: Option<&MarketResponse>,
    predicate: impl Fn(f64, f64) -> bool,
) -> bool {
    let outcome = step
        .params
        .get("outcome")
        .and_then(Value::as_str)
        .unwrap_or("YES");
    market_price(market, outcome)
        .zip(number_param(&step.params, "price"))
        .is_some_and(|(current, target)| predicate(current, target))
}

fn market_token_id(market: Option<&MarketResponse>, outcome: &str) -> Option<String> {
    market?
        .outcomes
        .iter()
        .find(|candidate| candidate.label.eq_ignore_ascii_case(outcome))?
        .id
        .clone()
}

fn market_price(market: Option<&MarketResponse>, outcome: &str) -> Option<f64> {
    market?
        .outcomes
        .iter()
        .find(|candidate| candidate.label.eq_ignore_ascii_case(outcome))?
        .price
}

fn schedule_seconds(value: &str) -> Option<i64> {
    match value {
        "5m" => Some(300),
        "15m" => Some(900),
        "30m" => Some(1_800),
        "1h" => Some(3_600),
        "4h" => Some(14_400),
        "12h" => Some(43_200),
        "24h" => Some(86_400),
        _ => None,
    }
}

fn number_param(value: &Value, key: &str) -> Option<f64> {
    number_value(value.get(key)?)
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ordered_workflow_steps, schedule_seconds};
    use crate::automations::dto::WorkflowPayload;

    #[test]
    fn orders_linear_workflow_from_connections() {
        let workflow: WorkflowPayload = serde_json::from_value(json!({
            "version": 1,
            "steps": [
                { "id": "action", "kind": "ACTION", "action": "SEND_MESSAGE", "params": {} },
                { "id": "trigger", "kind": "TRIGGER", "action": "TRIGGER_TIME_CHECK", "params": {} },
                { "id": "condition", "kind": "CONDITION", "action": "CONDITION_VOLUME_ABOVE", "params": {} }
            ],
            "connections": [
                { "from": "condition", "to": "action" },
                { "from": "trigger", "to": "condition" }
            ]
        }))
        .unwrap();

        let ordered = ordered_workflow_steps(&workflow).unwrap();
        let ids = ordered
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["trigger", "condition", "action"]);
    }

    #[test]
    fn rejects_branching_workflow() {
        let workflow: WorkflowPayload = serde_json::from_value(json!({
            "version": 1,
            "steps": [
                { "id": "trigger", "kind": "TRIGGER", "action": "TRIGGER_TIME_CHECK", "params": {} },
                { "id": "one", "kind": "ACTION", "action": "SEND_MESSAGE", "params": {} },
                { "id": "two", "kind": "ACTION", "action": "SEND_MESSAGE", "params": {} }
            ],
            "connections": [
                { "from": "trigger", "to": "one" },
                { "from": "trigger", "to": "two" }
            ]
        }))
        .unwrap();

        assert!(ordered_workflow_steps(&workflow).is_err());
    }

    #[test]
    fn supports_only_known_schedule_intervals() {
        assert_eq!(schedule_seconds("1h"), Some(3_600));
        assert_eq!(schedule_seconds("hourly"), None);
    }
}
