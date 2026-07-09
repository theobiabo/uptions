use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    automations::dto::{AutomationMarketPayload, WorkflowActionType, WorkflowPayload},
    polymarket::dto::MarketsQuery,
};

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
    pub market: AutomationMarketPayload,
    pub title: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAutomationToolPayload {
    pub automation_id: String,
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
    pub market: AutomationMarketPayload,
    pub title: String,
    pub workflow: WorkflowPayload,
}

#[derive(Debug, Deserialize)]
pub struct SearchMarketsPayload {
    pub active: Option<bool>,
    pub archived: Option<bool>,
    pub closed: Option<bool>,
    pub id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub slug: Option<String>,
}

impl From<SearchMarketsPayload> for MarketsQuery {
    fn from(payload: SearchMarketsPayload) -> Self {
        Self {
            active: payload.active,
            archived: payload.archived,
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
    pub market_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PrepareTradeActionPayload {
    pub action: WorkflowActionType,
    pub amount: f64,
    pub market: AutomationMarketPayload,
    pub order_type: String,
    pub outcome: String,
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}
