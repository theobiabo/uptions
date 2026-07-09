use axum::http::HeaderMap;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    automations::dto::{
        AutomationProvider, AutomationStatus, PublishAutomationRequest, TestRunAutomationRequest,
        WorkflowActionType,
    },
    entities::venue_connection,
    error::AppError,
    mcp::dto::{
        AutomationIdPayload, AutomationToolPayload, MarketIdPayload, McpRequest,
        PrepareTradeActionPayload, PromptGetParams, ResourceReadParams, SearchMarketsPayload,
        TestRunAutomationToolPayload, ToolCallParams, UpdateAutomationToolPayload,
    },
};

pub async fn handle_message(state: &AppState, headers: &HeaderMap, message: Value) -> Value {
    if let Value::Array(items) = message {
        if items.is_empty() {
            return jsonrpc_error(None, -32600, "Invalid empty JSON-RPC batch");
        }

        let mut responses = Vec::with_capacity(items.len());

        for item in items {
            responses.push(handle_single_message(state, headers, item).await);
        }

        return Value::Array(responses);
    }

    handle_single_message(state, headers, message).await
}

async fn handle_single_message(state: &AppState, headers: &HeaderMap, message: Value) -> Value {
    let request = match serde_json::from_value::<McpRequest>(message) {
        Ok(request) => request,
        Err(error) => return jsonrpc_error(None, -32600, format!("Invalid request: {error}")),
    };
    let id = request.id.clone();
    let result = match request.method.as_str() {
        "initialize" => Ok(initialize_result()),
        "ping" | "notifications/initialized" => Ok(json!({})),
        "tools/list" => Ok(tools_list_result()),
        "tools/call" => call_tool(state, headers, request.params).await,
        "resources/list" => Ok(resources_list_result()),
        "resources/read" => read_resource(state, headers, request.params).await,
        "prompts/list" => Ok(prompts_list_result()),
        "prompts/get" => get_prompt(request.params),
        _ => Err(AppError::BadRequest(format!(
            "Unsupported MCP method {}",
            request.method
        ))),
    };

    match result {
        Ok(result) => jsonrpc_success(id, result),
        Err(error) => app_error_response(id, error),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "uptions-mcp",
            "version": "1.0.0"
        }
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            tool("search_markets", "Search markets with Polymarket filters.", search_markets_schema()),
            tool("get_market", "Fetch one market by id.", market_id_schema()),
            tool("analyze_market", "Return structured market context for LLM analysis.", market_id_schema()),
            tool("list_automations", "List the authenticated user's automations.", empty_schema()),
            tool("get_automation", "Fetch one authenticated user automation.", automation_id_schema()),
            tool("create_automation", "Create an automation with a validated workflow.", automation_payload_schema()),
            tool("update_automation", "Update an existing automation with a validated workflow.", update_automation_payload_schema()),
            tool("test_run_automation", "Dry-run an automation workflow and create an alert.", test_run_payload_schema()),
            tool("pause_automation", "Pause an automation.", automation_id_schema()),
            tool("resume_automation", "Resume an automation.", automation_id_schema()),
            tool("delete_automation", "Delete an automation.", automation_id_schema()),
            tool("list_alerts", "List recent automation alerts.", empty_schema()),
            tool("prepare_trade_action", "Prepare a trade action preview without executing an order.", prepare_trade_action_schema())
        ]
    })
}

async fn call_tool(
    state: &AppState,
    headers: &HeaderMap,
    params: Value,
) -> Result<Value, AppError> {
    let params: ToolCallParams = parse_params(params, "Invalid tool call params")?;

    match params.name.as_str() {
        "search_markets" => {
            let args: SearchMarketsPayload =
                parse_params(params.arguments, "Invalid market search arguments")?;
            let markets = state.polymarket_client.fetch_markets(&args.into()).await?;
            Ok(tool_result(json!({ "markets": markets })))
        }
        "get_market" => {
            let args: MarketIdPayload = parse_params(params.arguments, "Invalid market arguments")?;
            let market = state
                .polymarket_client
                .fetch_market(&args.market_id)
                .await?;
            Ok(tool_result(json!({ "market": market })))
        }
        "analyze_market" => {
            let args: MarketIdPayload =
                parse_params(params.arguments, "Invalid market analysis arguments")?;
            let provider = AutomationProvider::default();
            let market = state
                .polymarket_client
                .fetch_market(&args.market_id)
                .await?;
            Ok(tool_result(market_analysis(provider, market)))
        }
        "list_automations" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let automations = state.automation_service.list(&user_id).await?;
            Ok(tool_result(json!({ "automations": automations })))
        }
        "get_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: AutomationIdPayload =
                parse_params(params.arguments, "Invalid automation arguments")?;
            let automation = state
                .automation_service
                .get(&user_id, &args.automation_id)
                .await?;
            Ok(tool_result(json!({ "automation": automation })))
        }
        "create_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: AutomationToolPayload =
                parse_params(params.arguments, "Invalid automation payload")?;
            let automation = state
                .automation_service
                .publish(&user_id, publish_request(args))
                .await?;
            Ok(tool_result(json!({ "automation": automation })))
        }
        "update_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: UpdateAutomationToolPayload =
                parse_params(params.arguments, "Invalid automation update payload")?;
            let automation_id = args.automation_id.clone();
            let automation = state
                .automation_service
                .update(&user_id, &automation_id, publish_request(args.into()))
                .await?;
            Ok(tool_result(json!({ "automation": automation })))
        }
        "test_run_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: TestRunAutomationToolPayload =
                parse_params(params.arguments, "Invalid test run payload")?;
            let result = state
                .automation_service
                .test_run(&user_id, test_run_request(args))
                .await?;
            Ok(tool_result(json!({ "test_run": result })))
        }
        "pause_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: AutomationIdPayload =
                parse_params(params.arguments, "Invalid automation arguments")?;
            let automation = state
                .automation_service
                .set_status(&user_id, &args.automation_id, AutomationStatus::Paused)
                .await?;
            Ok(tool_result(json!({ "automation": automation })))
        }
        "resume_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: AutomationIdPayload =
                parse_params(params.arguments, "Invalid automation arguments")?;
            let automation = state
                .automation_service
                .set_status(&user_id, &args.automation_id, AutomationStatus::Active)
                .await?;
            Ok(tool_result(json!({ "automation": automation })))
        }
        "delete_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: AutomationIdPayload =
                parse_params(params.arguments, "Invalid automation arguments")?;
            state
                .automation_service
                .delete(&user_id, &args.automation_id)
                .await?;
            Ok(tool_result(json!({ "deleted": true })))
        }
        "list_alerts" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let alerts = state.automation_service.alerts(&user_id).await?;
            Ok(tool_result(json!({ "alerts": alerts })))
        }
        "prepare_trade_action" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let args: PrepareTradeActionPayload =
                parse_params(params.arguments, "Invalid trade action payload")?;
            prepare_trade_action(state, &user_id, args).await
        }
        _ => Err(AppError::BadRequest(format!(
            "Unsupported MCP tool {}",
            params.name
        ))),
    }
}

fn resources_list_result() -> Value {
    let provider = AutomationProvider::default();

    json!({
        "resources": [
            {
                "uri": format!("markets://{}", provider.venue_id()),
                "name": format!("{} markets", provider.label()),
                "mimeType": "application/json"
            },
            {
                "uri": "automations://list",
                "name": "User automations",
                "mimeType": "application/json"
            },
            {
                "uri": "alerts://recent",
                "name": "Recent automation alerts",
                "mimeType": "application/json"
            }
        ]
    })
}

async fn read_resource(
    state: &AppState,
    headers: &HeaderMap,
    params: Value,
) -> Result<Value, AppError> {
    let params: ResourceReadParams = parse_params(params, "Invalid resource read params")?;
    let provider = AutomationProvider::default();
    let market_prefix = format!("market://{}/", provider.venue_id());

    if let Some(market_id) = params.uri.strip_prefix(&market_prefix) {
        let market = state.polymarket_client.fetch_market(market_id).await?;
        return Ok(resource_result(&params.uri, json!({ "market": market })));
    }

    if params.uri == "automations://list" {
        let user_id = authenticated_user_id(state, headers).await?;
        let automations = state.automation_service.list(&user_id).await?;
        return Ok(resource_result(
            &params.uri,
            json!({ "automations": automations }),
        ));
    }

    if let Some(automation_id) = params.uri.strip_prefix("automation://") {
        let user_id = authenticated_user_id(state, headers).await?;
        let automation = state
            .automation_service
            .get(&user_id, automation_id)
            .await?;
        return Ok(resource_result(
            &params.uri,
            json!({ "automation": automation }),
        ));
    }

    if params.uri == "alerts://recent" {
        let user_id = authenticated_user_id(state, headers).await?;
        let alerts = state.automation_service.alerts(&user_id).await?;
        return Ok(resource_result(&params.uri, json!({ "alerts": alerts })));
    }

    Err(AppError::NotFound("MCP resource not found".to_owned()))
}

fn prompts_list_result() -> Value {
    json!({
        "prompts": [
            {
                "name": "analyze_market_opportunity",
                "description": "Analyze a market with available MCP market context.",
                "arguments": [{ "name": "market_id", "required": true }]
            },
            {
                "name": "build_automation_from_strategy",
                "description": "Convert a strategy into a valid Uptions workflow payload.",
                "arguments": [{ "name": "strategy", "required": true }]
            },
            {
                "name": "review_automation_before_publish",
                "description": "Review an automation payload in plain English before publishing.",
                "arguments": [{ "name": "automation", "required": true }]
            }
        ]
    })
}

fn get_prompt(params: Value) -> Result<Value, AppError> {
    let params: PromptGetParams = parse_params(params, "Invalid prompt params")?;
    let text = match params.name.as_str() {
        "analyze_market_opportunity" => {
            "Analyze the market using only provided market data. Explain observable pricing, volume, liquidity, uncertainty, automation opportunities, and risks. Do not claim certainty or place trades."
        }
        "build_automation_from_strategy" => {
            "Convert the user's strategy into a Uptions workflow payload with version, steps, and connections. Use trigger, condition, and action steps. Prefer message actions unless the user explicitly asks for buy or sell behavior."
        }
        "review_automation_before_publish" => {
            "Review the automation payload for correctness, risks, missing parameters, and plain-English behavior. Explain what will happen before the user publishes it."
        }
        _ => return Err(AppError::NotFound("MCP prompt not found".to_owned())),
    };

    Ok(json!({
        "description": params.name,
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": text
                }
            }
        ],
        "arguments": params.arguments
    }))
}

async fn prepare_trade_action(
    state: &AppState,
    user_id: &str,
    args: PrepareTradeActionPayload,
) -> Result<Value, AppError> {
    if !matches!(
        args.action,
        WorkflowActionType::Buy | WorkflowActionType::Sell
    ) {
        return Err(AppError::BadRequest(
            "trade action must be BUY or SELL".to_owned(),
        ));
    }

    if !args.amount.is_finite() || args.amount <= 0.0 {
        return Err(AppError::BadRequest("amount must be positive".to_owned()));
    }

    let outcome = normalize_choice(&args.outcome);
    let order_type = normalize_choice(&args.order_type);

    if !matches!(outcome.as_str(), "YES" | "NO") {
        return Err(AppError::BadRequest("outcome must be YES or NO".to_owned()));
    }

    if !matches!(order_type.as_str(), "MARKET" | "LIMIT") {
        return Err(AppError::BadRequest(
            "order_type must be MARKET or LIMIT".to_owned(),
        ));
    }

    let provider = AutomationProvider::default();
    let provider_ready = has_ready_provider_connection(state, user_id, provider).await?;

    Ok(tool_result(json!({
        "provider": provider,
        "venue": provider.venue_id(),
        "market": args.market,
        "action": args.action,
        "params": {
            "outcome": outcome,
            "order_type": order_type,
            "amount": args.amount
        },
        "provider_ready": provider_ready,
        "execution_status": "not_executed",
        "requires_user_confirmation": true,
        "summary": format!("Prepare to {:?} {outcome} with a {order_type} order for ${}.", args.action, args.amount),
        "risk_note": "This MCP tool prepares a trade action preview only. Use create_automation or update_automation to save workflow behavior after user review."
    })))
}

async fn has_ready_provider_connection(
    state: &AppState,
    user_id: &str,
    provider: AutomationProvider,
) -> Result<bool, AppError> {
    let connection = venue_connection::Entity::find()
        .filter(venue_connection::Column::UserId.eq(user_id))
        .filter(venue_connection::Column::Venue.eq(provider.venue_id()))
        .filter(venue_connection::Column::Enabled.eq(true))
        .filter(venue_connection::Column::Status.eq("active"))
        .one(&state.db)
        .await?;

    Ok(connection.is_some())
}

fn publish_request(payload: AutomationToolPayload) -> PublishAutomationRequest {
    PublishAutomationRequest {
        market: payload.market,
        provider: AutomationProvider::default(),
        title: payload.title,
        workflow: payload.workflow,
    }
}

fn test_run_request(payload: TestRunAutomationToolPayload) -> TestRunAutomationRequest {
    TestRunAutomationRequest {
        automation_id: payload.automation_id,
        market: payload.market,
        provider: AutomationProvider::default(),
        title: payload.title,
        workflow: payload.workflow,
    }
}

impl From<UpdateAutomationToolPayload> for AutomationToolPayload {
    fn from(payload: UpdateAutomationToolPayload) -> Self {
        Self {
            market: payload.market,
            title: payload.title,
            workflow: payload.workflow,
        }
    }
}

fn market_analysis(provider: AutomationProvider, market: Value) -> Value {
    json!({
        "provider": provider,
        "venue": provider.venue_id(),
        "venue_label": provider.label(),
        "title": first_text(&market, &["question", "title"]),
        "description": first_text(&market, &["description"]),
        "outcomes": market_value(&market, "outcomes"),
        "outcome_prices": market_value(&market, "outcomePrices"),
        "volume": market_value(&market, "volume"),
        "volume_num": market_value(&market, "volumeNum"),
        "liquidity": market_value(&market, "liquidity"),
        "last_trade_price": market_value(&market, "lastTradePrice"),
        "best_bid": market_value(&market, "bestBid"),
        "best_ask": market_value(&market, "bestAsk"),
        "one_day_price_change": market_value(&market, "oneDayPriceChange"),
        "automation_ideas": [
            "Watch price movement and notify when conditions match.",
            "Add a price threshold condition before any buy or sell action.",
            "Use test_run_automation before publishing."
        ],
        "risk_notes": [
            "Prediction markets are volatile and can resolve unexpectedly.",
            "Trade actions require a connected venue account and user-reviewed workflow settings.",
            "This analysis is based on market data only and is not financial advice."
        ],
        "raw_market": market
    })
}

fn market_value(market: &Value, key: &str) -> Value {
    market.get(key).cloned().unwrap_or(Value::Null)
}

fn first_text(market: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        market
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn normalize_choice(value: &str) -> String {
    value.trim().replace('-', "_").to_ascii_uppercase()
}

fn parse_params<T>(params: Value, message: &str) -> Result<T, AppError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(params)
        .map_err(|error| AppError::BadRequest(format!("{message}: {error}")))
}

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn search_markets_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string" },
            "slug": { "type": "string" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 100 },
            "offset": { "type": "integer", "minimum": 0 },
            "active": { "type": "boolean" },
            "closed": { "type": "boolean" },
            "archived": { "type": "boolean" }
        }
    })
}

fn market_id_schema() -> Value {
    json!({
        "type": "object",
        "required": ["market_id"],
        "additionalProperties": false,
        "properties": {
            "market_id": { "type": "string" }
        }
    })
}

fn automation_id_schema() -> Value {
    json!({
        "type": "object",
        "required": ["automation_id"],
        "additionalProperties": false,
        "properties": {
            "automation_id": { "type": "string" }
        }
    })
}

fn automation_payload_schema() -> Value {
    json!({
        "type": "object",
        "required": ["title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn update_automation_payload_schema() -> Value {
    json!({
        "type": "object",
        "required": ["automation_id", "title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "automation_id": { "type": "string" },
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn test_run_payload_schema() -> Value {
    json!({
        "type": "object",
        "required": ["title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "automation_id": { "type": "string" },
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn prepare_trade_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["market", "action", "outcome", "order_type", "amount"],
        "additionalProperties": false,
        "properties": {
            "market": market_schema(),
            "action": { "type": "string", "enum": [WorkflowActionType::Buy, WorkflowActionType::Sell] },
            "outcome": { "type": "string", "enum": ["YES", "NO"] },
            "order_type": { "type": "string", "enum": ["MARKET", "LIMIT"] },
            "amount": { "type": "number", "exclusiveMinimum": 0 }
        }
    })
}

fn market_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "title"],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string" },
            "title": { "type": "string" }
        }
    })
}

fn workflow_schema() -> Value {
    json!({
        "type": "object",
        "required": ["version", "steps", "connections"],
        "additionalProperties": false,
        "properties": {
            "version": { "type": "integer", "const": 1 },
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "kind", "action", "params"],
                    "additionalProperties": false,
                    "properties": {
                        "id": { "type": "string" },
                        "kind": { "type": "string", "enum": ["TRIGGER", "CONDITION", "ACTION"] },
                        "action": { "type": "string" },
                        "params": { "type": "object" }
                    }
                }
            },
            "connections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["from", "to"],
                    "additionalProperties": false,
                    "properties": {
                        "from": { "type": "string" },
                        "to": { "type": "string" }
                    }
                }
            }
        }
    })
}

fn tool_result(value: Value) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": pretty_json(&value)
            }
        ],
        "structuredContent": value
    })
}

fn resource_result(uri: &str, value: Value) -> Value {
    json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": "application/json",
                "text": pretty_json(&value)
            }
        ]
    })
}

fn jsonrpc_success(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result
    })
}

fn jsonrpc_error(id: Option<Value>, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

fn app_error_response(id: Option<Value>, error: AppError) -> Value {
    let code = match error {
        AppError::Unauthorized => -32001,
        AppError::BadRequest(_) => -32602,
        AppError::Conflict(_) => -32009,
        AppError::NotFound(_) => -32004,
        AppError::ExternalApiError(_) => -32052,
        AppError::DatabaseError(_) => -32603,
    };

    jsonrpc_error(id, code, error.to_string())
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}
