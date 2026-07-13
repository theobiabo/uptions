use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomationProvider {
    Polymarket,
}

impl AutomationProvider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Polymarket => "Polymarket",
        }
    }

    pub fn venue_id(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
        }
    }
}

impl Default for AutomationProvider {
    fn default() -> Self {
        Self::Polymarket
    }
}

impl<'de> Deserialize<'de> for AutomationProvider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        match normalize_enum_value(&value).as_str() {
            "POLYMARKET" => Ok(Self::Polymarket),
            _ => Err(de::Error::custom("provider is invalid")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomationStepKind {
    Trigger,
    Condition,
    Action,
}

impl<'de> Deserialize<'de> for AutomationStepKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        match normalize_enum_value(&value).as_str() {
            "TRIGGER" => Ok(Self::Trigger),
            "CONDITION" => Ok(Self::Condition),
            "ACTION" => Ok(Self::Action),
            _ => Err(de::Error::custom("workflow step kind is invalid")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
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

impl<'de> Deserialize<'de> for WorkflowActionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        match normalize_enum_value(&value).as_str() {
            "TRIGGERPRICEMOVES" | "PRICEMOVES" => Ok(Self::TriggerPriceMoves),
            "TRIGGERVOLUMEMOVES" | "VOLUMEMOVES" => Ok(Self::TriggerVolumeMoves),
            "TRIGGERTIMECHECK" | "TIMECHECK" => Ok(Self::TriggerTimeCheck),
            "CONDITIONOUTCOMEPRICEABOVE" | "OUTCOMEPRICEABOVE" => {
                Ok(Self::ConditionOutcomePriceAbove)
            }
            "CONDITIONOUTCOMEPRICEBELOW" | "OUTCOMEPRICEBELOW" => {
                Ok(Self::ConditionOutcomePriceBelow)
            }
            "CONDITIONVOLUMEABOVE" | "VOLUMEABOVE" => Ok(Self::ConditionVolumeAbove),
            "BUY" | "BUYOUTCOME" => Ok(Self::Buy),
            "SELL" | "SELLOUTCOME" => Ok(Self::Sell),
            "SENDMESSAGE" => Ok(Self::SendMessage),
            _ => Err(de::Error::custom("workflow action is invalid")),
        }
    }
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
    #[serde(default)]
    pub provider: AutomationProvider,
    pub workflow: WorkflowPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AutomationStatus {
    Active,
    Paused,
}

impl<'de> Deserialize<'de> for AutomationStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        match normalize_enum_value(&value).as_str() {
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" | "STOPPED" | "INACTIVE" => Ok(Self::Paused),
            _ => Err(de::Error::custom("automation status is invalid")),
        }
    }
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
    #[serde(default)]
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
    #[schema(example = "2026-07-07T12:35:00Z")]
    pub read_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MarkAlertsReadResponse {
    #[schema(example = 3)]
    pub updated: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClearAlertsResponse {
    #[schema(example = 3)]
    pub deleted: u64,
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

fn normalize_enum_value(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
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
                            "order_type": "LIMIT",
                            "usdc_amount": 10,
                            "limit_price": 0.55
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

    #[test]
    fn defaults_provider_when_omitted() {
        let payload = serde_json::json!({
            "title": "Backend default provider",
            "market": {
                "id": "540818",
                "title": "Backend default provider"
            },
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
                        "id": "send-message-def",
                        "kind": "ACTION",
                        "action": "SEND_MESSAGE",
                        "params": {
                            "channel": "IN_APP",
                            "message": "Market condition met"
                        }
                    }
                ],
                "connections": [
                    {
                        "from": "price-moves-abc",
                        "to": "send-message-def"
                    }
                ]
            }
        });

        let request: PublishAutomationRequest = serde_json::from_value(payload).unwrap();

        assert_eq!(request.provider, AutomationProvider::Polymarket);
    }

    #[test]
    fn deserializes_runtime_enum_variants() {
        let payload = serde_json::json!({
            "title": "Runtime payload",
            "market": {
                "id": "540818",
                "title": "Runtime payload"
            },
            "provider": "polymarket",
            "workflow": {
                "version": 1,
                "steps": [
                    {
                        "id": "price-moves-abc",
                        "kind": "trigger",
                        "action": "price-moves",
                        "params": {
                            "outcome": "YES"
                        }
                    },
                    {
                        "id": "send-message-def",
                        "kind": "action",
                        "action": "send-message",
                        "params": {
                            "channel": "IN_APP",
                            "message": "Market condition met"
                        }
                    }
                ],
                "connections": [
                    {
                        "from": "price-moves-abc",
                        "to": "send-message-def"
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
        assert_eq!(request.workflow.steps[1].kind, AutomationStepKind::Action);
        assert_eq!(
            request.workflow.steps[1].action,
            WorkflowActionType::SendMessage
        );
    }
}
