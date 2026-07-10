use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{Value, json};
use tokio::time::{self, Duration as TokioDuration};
use uuid::Uuid;

use crate::{
    automations::{
        dto::{AutomationStepKind, WorkflowActionType, WorkflowPayload, WorkflowStepPayload},
        service::AutomationService,
    },
    db::Db,
    entities::{automation, automation_run},
    error::AppError,
    polymarket::client::PolymarketClient,
};

const AUTOMATION_INTERVAL_SECONDS: u64 = 30;
const AUTOMATION_COOLDOWN_SECONDS: i64 = 300;

#[derive(Clone)]
pub struct AutomationExecutor {
    automation_service: AutomationService,
    db: Db,
    polymarket_client: PolymarketClient,
}

impl AutomationExecutor {
    pub fn new(
        db: Db,
        automation_service: AutomationService,
        polymarket_client: PolymarketClient,
    ) -> Self {
        Self {
            automation_service,
            db,
            polymarket_client,
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
        if self.is_in_cooldown(&automation.id).await? {
            return Ok(());
        }

        let workflow = serde_json::from_value::<WorkflowPayload>(automation.workflow.clone())
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let market = self.fetch_market(&automation).await;
        let market_value = market.as_ref().ok();
        let triggers = workflow_steps(&workflow, AutomationStepKind::Trigger);
        let conditions = workflow_steps(&workflow, AutomationStepKind::Condition);
        let actions = workflow_steps(&workflow, AutomationStepKind::Action);
        let trigger_snapshot = json!({
            "matched": !triggers.is_empty(),
            "steps": triggers,
            "market_id": automation.market_id,
            "market_title": automation.market_title
        });
        let condition_snapshot = json!({
            "matched": conditions.iter().all(|step| condition_matches(step, market_value)),
            "steps": conditions
        });

        if !trigger_snapshot["matched"].as_bool().unwrap_or(false)
            || !condition_snapshot["matched"].as_bool().unwrap_or(false)
            || actions.is_empty()
        {
            self.create_run(
                &automation,
                "skipped",
                trigger_snapshot,
                condition_snapshot,
                json!({ "actions": actions }),
                Some(json!({ "reason": "workflow did not match" })),
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
                .execute_action(&automation, &run_id, action, market_value)
                .await?;

            if result
                .get("approval_required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                status = "approval_required";
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
        .await?;

        Ok(())
    }

    async fn execute_action(
        &self,
        automation: &automation::Model,
        run_id: &str,
        action: &WorkflowStepPayload,
        market: Option<&Value>,
    ) -> Result<Value, AppError> {
        match action.action {
            WorkflowActionType::SendMessage => {
                self.send_message_action(automation, run_id, action).await
            }
            WorkflowActionType::Buy | WorkflowActionType::Sell => {
                self.trade_approval_action(automation, run_id, action, market)
                    .await
            }
            _ => Ok(json!({ "skipped": true, "action": action.action })),
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
        market: Option<&Value>,
    ) -> Result<Value, AppError> {
        let side = match action.action {
            WorkflowActionType::Buy => "BUY",
            WorkflowActionType::Sell => "SELL",
            _ => "UNKNOWN",
        };
        let outcome = action
            .params
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("YES");
        let amount = number_param(&action.params, "amount");
        let order_type = action
            .params
            .get("order_type")
            .and_then(Value::as_str)
            .unwrap_or("MARKET");
        let max_price = number_param(&action.params, "max_price")
            .or_else(|| number_param(&action.params, "price"))
            .or_else(|| market_price(market, outcome));
        let message = format!(
            "{} wants approval to {} {} on {}.",
            automation.title,
            side,
            outcome,
            automation.market_title.as_deref().unwrap_or("this market")
        );

        self.automation_service
            .create_alert(
                &automation.user_id,
                Some(automation.id.clone()),
                "Trade approval required",
                &message,
                "pending",
                json!({
                    "type": "automation_trade_approval_requested",
                    "automation_id": automation.id,
                    "run_id": run_id,
                    "market_id": automation.market_id,
                    "market_title": automation.market_title,
                    "side": side,
                    "outcome": outcome,
                    "amount": amount,
                    "order_type": order_type,
                    "max_price": max_price,
                    "reason": "Automation condition matched",
                    "step_id": action.id
                }),
            )
            .await?;

        Ok(json!({
            "action": "trade_approval",
            "approval_required": true,
            "side": side,
            "outcome": outcome,
            "amount": amount,
            "order_type": order_type,
            "max_price": max_price
        }))
    }

    async fn fetch_market(&self, automation: &automation::Model) -> Result<Value, AppError> {
        let market_id = automation
            .market_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("automation market id is missing".to_owned()))?;

        self.polymarket_client.fetch_market(market_id).await
    }

    async fn is_in_cooldown(&self, automation_id: &str) -> Result<bool, AppError> {
        let since = (Utc::now() - Duration::seconds(AUTOMATION_COOLDOWN_SECONDS)).fixed_offset();
        let run = automation_run::Entity::find()
            .filter(automation_run::Column::AutomationId.eq(automation_id))
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

        Ok(())
    }
}

fn workflow_steps(
    workflow: &WorkflowPayload,
    kind: AutomationStepKind,
) -> Vec<&WorkflowStepPayload> {
    workflow
        .steps
        .iter()
        .filter(|step| step.kind == kind)
        .collect()
}

fn condition_matches(step: &WorkflowStepPayload, market: Option<&Value>) -> bool {
    match step.action {
        WorkflowActionType::ConditionOutcomePriceAbove => {
            compare_price(step, market, |current, target| current > target)
        }
        WorkflowActionType::ConditionOutcomePriceBelow => {
            compare_price(step, market, |current, target| current < target)
        }
        WorkflowActionType::ConditionVolumeAbove => market
            .and_then(|value| number_from_keys(value, &["volumeNum", "volume"]))
            .zip(number_param(&step.params, "volume"))
            .is_some_and(|(current, target)| current > target),
        _ => true,
    }
}

fn compare_price(
    step: &WorkflowStepPayload,
    market: Option<&Value>,
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

fn market_price(market: Option<&Value>, outcome: &str) -> Option<f64> {
    let market = market?;
    let outcomes = string_array(market.get("outcomes"));
    let prices = number_array(market.get("outcomePrices"));
    let index = outcomes
        .iter()
        .position(|value| value.eq_ignore_ascii_case(outcome))
        .unwrap_or(0);

    prices
        .get(index)
        .copied()
        .or_else(|| number_from_keys(market, &["lastTradePrice", "bestAsk", "bestBid"]))
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(text)) => serde_json::from_str::<Vec<String>>(text).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn number_array(value: Option<&Value>) -> Vec<f64> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(number_value).collect(),
        Some(Value::String(text)) => serde_json::from_str::<Vec<Value>>(text)
            .map(|items| items.iter().filter_map(number_value).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn number_param(value: &Value, key: &str) -> Option<f64> {
    number_value(value.get(key)?)
}

fn number_from_keys(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| number_value(value.get(*key)?))
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}
