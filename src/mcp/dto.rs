use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use crate::{
    automations::dto::{AutomationMarketPayload, WorkflowActionType, WorkflowPayload},
    markets::types::MarketListQuery,
    providers::types::ProviderId,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpJsonRpcRequest {
    #[schema(example = "2.0")]
    pub jsonrpc: Option<String>,
    #[schema(value_type = Object)]
    pub id: Option<Value>,
    #[schema(example = "tools/list")]
    pub method: String,
    #[schema(value_type = Object)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpJsonRpcResponse {
    #[schema(example = "2.0")]
    pub jsonrpc: String,
    #[schema(value_type = Object)]
    pub id: Value,
    #[schema(value_type = Object)]
    pub result: Option<Value>,
    #[schema(value_type = Object)]
    pub error: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub id: Option<Value>,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: Value,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    #[serde(default = "empty_object")]
    pub arguments: Value,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ResourceReadParams {
    pub uri: String,
}

#[derive(Debug, Deserialize)]
pub struct PromptGetParams {
    #[serde(default = "empty_object")]
    pub arguments: Value,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AutomationToolPayload {
    pub provider: ProviderId,
    pub market: AutomationMarketPayload,
    pub title: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAutomationToolPayload {
    pub automation_id: String,
    pub provider: ProviderId,
    pub market: AutomationMarketPayload,
    pub title: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize)]
pub struct AutomationIdPayload {
    pub automation_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TestRunAutomationToolPayload {
    pub automation_id: Option<String>,
    pub provider: ProviderId,
    pub market: AutomationMarketPayload,
    pub title: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize)]
pub struct SearchMarketsPayload {
    pub provider: ProviderId,
    pub active: Option<bool>,
    pub archived: Option<bool>,
    pub closed: Option<bool>,
    pub id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub slug: Option<String>,
}

impl From<SearchMarketsPayload> for MarketListQuery {
    fn from(payload: SearchMarketsPayload) -> Self {
        Self {
            active: payload.active,
            archived: payload.archived,
            cursor: None,
            closed: payload.closed,
            id: payload.id,
            limit: payload.limit,
            offset: payload.offset,
            slug: payload.slug,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MarketIdPayload {
    pub provider: ProviderId,
    pub market_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PrepareTradeActionPayload {
    pub provider: ProviderId,
    pub action: WorkflowActionType,
    pub amount: f64,
    pub market: AutomationMarketPayload,
    pub order_type: String,
    pub outcome: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpApprovalResponse {
    pub id: String,
    pub tool: String,
    pub status: String,
    #[schema(value_type = Object)]
    pub payload: Value,
    #[schema(value_type = Object)]
    pub result: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub decided_at: Option<String>,
    pub expires_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpApprovalDecisionResponse {
    pub approval: McpApprovalResponse,
    #[schema(value_type = Object)]
    pub result: Option<Value>,
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}
