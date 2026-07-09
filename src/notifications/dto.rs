use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AutomationAlertStreamEvent {
    #[schema(example = "a861ae59-36e9-4ee0-b3c0-fd8f44bb69b7")]
    pub id: String,
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub automation_id: Option<String>,
    #[schema(example = "automation_alert")]
    pub event_type: String,
    #[schema(example = "Automation paused")]
    pub title: String,
    #[schema(example = "Your automation was paused.")]
    pub message: String,
    #[schema(example = "info")]
    pub status: String,
    #[schema(value_type = Object)]
    pub meta: Value,
    #[schema(example = "2026-07-09T12:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-07-09T12:35:00Z")]
    pub read_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UserNotificationEvent {
    pub user_id: String,
    pub alert: AutomationAlertStreamEvent,
}
