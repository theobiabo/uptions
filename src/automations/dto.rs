use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowPayload {
    pub edges: Vec<WorkflowEdgePayload>,
    pub nodes: Vec<WorkflowNodePayload>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowNodePayload {
    pub data: WorkflowBlockPayload,
    #[schema(example = "trigger-1")]
    pub id: String,
    pub position: WorkflowPositionPayload,
    #[schema(example = "workflowBlock")]
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowBlockPayload {
    #[schema(example = "Watch a market price threshold")]
    pub description: String,
    #[schema(example = "price-trigger")]
    pub id: String,
    #[schema(example = "trigger")]
    pub kind: String,
    #[schema(example = "Price trigger")]
    pub title: String,
    #[schema(example = "yes >= 0.65")]
    pub value: String,
    #[schema(example = "polymarket")]
    pub venue: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowPositionPayload {
    #[schema(example = 120.0)]
    pub x: f64,
    #[schema(example = 160.0)]
    pub y: f64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkflowEdgePayload {
    #[schema(example = "edge-1")]
    pub id: String,
    #[schema(example = "trigger-1")]
    pub source: String,
    #[schema(example = "action-1")]
    pub target: String,
    #[schema(example = "smoothstep")]
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PublishAutomationRequest {
    #[schema(example = "Will BTC close above $100k?")]
    pub title: String,
    #[schema(example = "540817")]
    pub market_id: Option<String>,
    #[schema(example = "Will Bitcoin close above $100,000 this year?")]
    pub market_title: Option<String>,
    #[schema(example = "polymarket")]
    pub venue: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TestRunAutomationRequest {
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub automation_id: Option<String>,
    #[schema(example = "Will BTC close above $100k?")]
    pub title: String,
    #[schema(example = "540817")]
    pub market_id: Option<String>,
    #[schema(example = "Will Bitcoin close above $100,000 this year?")]
    pub market_title: Option<String>,
    #[schema(example = "polymarket")]
    pub venue: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AutomationResponse {
    #[schema(example = "4adcf640-7784-4d0d-b921-5066e4f9057b")]
    pub id: String,
    #[schema(example = "Will BTC close above $100k?")]
    pub title: String,
    #[schema(example = "540817")]
    pub market_id: Option<String>,
    #[schema(example = "Will Bitcoin close above $100,000 this year?")]
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
        example = "Test run completed successfully for Will BTC close above $100k? with 1 workflow blocks."
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
        example = "Test run completed successfully for Will BTC close above $100k? with 1 workflow blocks."
    )]
    pub message: String,
    #[schema(example = 1)]
    pub checked_blocks: usize,
    pub alert: AutomationAlertResponse,
}
