use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomationProvider {
    Polymarket,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomationStepKind {
    Trigger,
    Condition,
    Action,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowActionType {
    TriggerPriceMoves,
    TriggerVolumeMoves,
    TriggerTimeCheck,
    ConditionOutcomePriceAbove,
    ConditionOutcomePriceBelow,
    ConditionVolumeAbove,
    Buy,
    Sell,
    SendMessage,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AutomationMarketPayload {
    #[schema(example = "540818")]
    pub id: String,
    #[schema(example = "New Playboi Carti Album before GTA VI?")]
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowPayload {
    #[schema(example = 1)]
    pub version: u16,
    pub steps: Vec<WorkflowStepPayload>,
    pub connections: Vec<WorkflowConnectionPayload>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowStepPayload {
    #[schema(example = "price-moves-123")]
    pub id: String,
    pub kind: AutomationStepKind,
    pub action: WorkflowActionType,
    #[schema(value_type = Object)]
    pub params: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowConnectionPayload {
    #[schema(example = "price-moves-123")]
    pub from: String,
    #[schema(example = "outcome-price-above-456")]
    pub to: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PublishAutomationRequest {
    #[schema(example = "New Playboi Carti Album before GTA VI?")]
    pub title: String,
    pub market: AutomationMarketPayload,
    pub provider: AutomationProvider,
    pub workflow: WorkflowPayload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AutomationStatus {
    Active,
    Paused,
}

impl AutomationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAutomationStatusRequest {
    pub status: AutomationStatus,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TestRunAutomationRequest {
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub automation_id: Option<String>,
    #[schema(example = "New Playboi Carti Album before GTA VI?")]
    pub title: String,
    pub market: AutomationMarketPayload,
    pub provider: AutomationProvider,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AutomationResponse {
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub id: String,
    #[schema(example = "New Playboi Carti Album before GTA VI?")]
    pub title: String,
    #[schema(example = "540818")]
    pub market_id: Option<String>,
    #[schema(example = "New Playboi Carti Album before GTA VI?")]
    pub market_title: Option<String>,
    #[schema(example = "polymarket")]
    pub venue: String,
    #[schema(example = "active")]
    pub status: String,
    #[schema(value_type = WorkflowPayload)]
    pub workflow: Value,
    #[schema(example = "success")]
    pub last_run_status: Option<String>,
    #[schema(example = "2026-07-07T12:30:00Z")]
    pub last_run_at: Option<String>,
    #[schema(example = "2026-07-07T12:00:00Z")]
    pub created_at: String,
    #[schema(example = "2026-07-07T12:30:00Z")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AutomationAlertResponse {
    #[schema(example = "a861ae59-36e9-4ee0-b3c0-fd8f44bb69b7")]
    pub id: String,
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub automation_id: Option<String>,
    #[schema(example = "Test run completed")]
    pub title: String,
    #[schema(
        example = "Test run completed successfully for New Playboi Carti Album before GTA VI? with 3 workflow steps."
    )]
    pub message: String,
    #[schema(example = "success")]
    pub status: String,
    #[schema(value_type = Object)]
    pub meta: Value,
    #[schema(example = "2026-07-07T12:30:00Z")]
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestRunAutomationResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(
        example = "Test run completed successfully for New Playboi Carti Album before GTA VI? with 3 workflow steps."
    )]
    pub message: String,
    #[schema(example = 3)]
    pub checked_blocks: usize,
    pub alert: AutomationAlertResponse,
}

#[cfg(test)]
mod tests {
    use super::{AutomationStatus, PublishAutomationRequest, UpdateAutomationStatusRequest};
    use crate::automations::dto::{AutomationProvider, AutomationStepKind, WorkflowActionType};

    #[test]
    fn deserializes_frontend_publish_payload() {
        let payload = serde_json::json!({
            "title": "New Playboi Carti Album before GTA VI?",
            "market": {
                "id": "540818",
                "title": "New Playboi Carti Album before GTA VI?"
            },
            "provider": "POLYMARKET",
            "workflow": {
                "version": 1,
                "steps": [
                    {
                        "id": "price-moves-abc",
                        "kind": "TRIGGER",
                        "action": "TRIGGER_PRICE_MOVES",
                        "params": {
                            "outcome": "YES"
                        }
                    },
                    {
                        "id": "buy-ghi",
                        "kind": "ACTION",
                        "action": "BUY",
                        "params": {
                            "outcome": "YES",
                            "order_type": "MARKET",
                            "amount": 10
                        }
                    }
                ],
                "connections": [
                    {
                        "from": "price-moves-abc",
                        "to": "buy-ghi"
                    }
                ]
            }
        });

        let request: PublishAutomationRequest = serde_json::from_value(payload).unwrap();

        assert_eq!(request.provider, AutomationProvider::Polymarket);
        assert_eq!(request.workflow.steps[0].kind, AutomationStepKind::Trigger);
        assert_eq!(
            request.workflow.steps[0].action,
            WorkflowActionType::TriggerPriceMoves
        );
    }

    #[test]
    fn deserializes_frontend_status_payload() {
        let payload = serde_json::json!({ "status": "paused" });
        let request: UpdateAutomationStatusRequest = serde_json::from_value(payload).unwrap();

        assert_eq!(request.status, AutomationStatus::Paused);
    }
}
