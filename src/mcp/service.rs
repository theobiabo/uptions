use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    automations::dto::{
        AutomationStatus, PublishAutomationRequest, TestRunAutomationRequest, WorkflowActionType,
    },
    entities::{automation_alert, mcp_approval_request, venue_connection},
    error::AppError,
    markets::types::MarketResponse,
    mcp::dto::{
        AutomationIdPayload, AutomationToolPayload, MarketIdPayload, McpApprovalDecisionResponse,
        McpApprovalResponse, McpRequest, PrepareTradeActionPayload, PromptGetParams,
        ResourceReadParams, SearchMarketsPayload, TestRunAutomationToolPayload, ToolCallParams,
        UpdateAutomationToolPayload,
    },
    providers::types::{DEFAULT_PROVIDER, ProviderId},
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
            let provider = args.provider;
            let markets = state
                .providers
                .fetch_markets(provider, &args.into())
                .await?;
            Ok(tool_result(json!({ "markets": markets })))
        }
        "get_market" => {
            let args: MarketIdPayload = parse_params(params.arguments, "Invalid market arguments")?;
            let market = state
                .providers
                .fetch_market(args.provider, &args.market_id)
                .await?;
            Ok(tool_result(
                json!({ "provider": args.provider, "market": market }),
            ))
        }
        "analyze_market" => {
            let args: MarketIdPayload =
                parse_params(params.arguments, "Invalid market analysis arguments")?;
            let provider = args.provider;
            let market = state
                .providers
                .fetch_market(provider, &args.market_id)
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
            let _: AutomationToolPayload =
                parse_params(params.arguments.clone(), "Invalid automation payload")?;
            create_approval_request(state, &user_id, "create_automation", params.arguments).await
        }
        "update_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let _: UpdateAutomationToolPayload = parse_params(
                params.arguments.clone(),
                "Invalid automation update payload",
            )?;
            create_approval_request(state, &user_id, "update_automation", params.arguments).await
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
            let _: AutomationIdPayload =
                parse_params(params.arguments.clone(), "Invalid automation arguments")?;
            create_approval_request(state, &user_id, "pause_automation", params.arguments).await
        }
        "resume_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let _: AutomationIdPayload =
                parse_params(params.arguments.clone(), "Invalid automation arguments")?;
            create_approval_request(state, &user_id, "resume_automation", params.arguments).await
        }
        "delete_automation" => {
            let user_id = authenticated_user_id(state, headers).await?;
            let _: AutomationIdPayload =
                parse_params(params.arguments.clone(), "Invalid automation arguments")?;
            create_approval_request(state, &user_id, "delete_automation", params.arguments).await
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
    let provider = DEFAULT_PROVIDER;

    json!({
        "resources": [
            {
                "uri": format!("markets://{}", provider.route_value()),
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
    if let Some(resource) = params.uri.strip_prefix("market://") {
        let (provider, market_id) = resource
            .split_once('/')
            .ok_or_else(|| AppError::BadRequest("MCP market resource URI is invalid".to_owned()))?;
        let provider = provider
            .parse::<ProviderId>()
            .map_err(|message| AppError::BadRequest(message.to_owned()))?;
        let market = state.providers.fetch_market(provider, market_id).await?;
        return Ok(resource_result(
            &params.uri,
            json!({ "provider": provider, "market": market }),
        ));
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

    let provider = args.provider;
    let provider_ready = has_ready_provider_connection(state, user_id, provider).await?;

    Ok(tool_result(json!({
        "provider": provider,
        "venue": provider.route_value(),
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

pub async fn list_approvals(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<McpApprovalResponse>, AppError> {
    let approvals = mcp_approval_request::Entity::find()
        .filter(mcp_approval_request::Column::UserId.eq(user_id))
        .order_by_desc(mcp_approval_request::Column::CreatedAt)
        .all(&state.db)
        .await?;

    Ok(approvals.into_iter().map(approval_response).collect())
}

pub async fn get_approval(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
) -> Result<McpApprovalResponse, AppError> {
    let approval = find_owned_approval(state, user_id, approval_id).await?;

    Ok(approval_response(approval))
}

pub async fn approve_request(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
) -> Result<McpApprovalDecisionResponse, AppError> {
    let approval = find_pending_approval(state, user_id, approval_id).await?;
    let result =
        execute_approved_tool(state, user_id, &approval.tool, approval.payload.clone()).await?;
    let now = Utc::now();
    let mut active = approval.into_active_model();
    active.status = Set("approved".to_owned());
    active.result = Set(Some(result.clone()));
    active.updated_at = Set(now.into());
    active.decided_at = Set(Some(now.into()));
    let approval = active.update(&state.db).await?;
    let response = approval_response(approval);
    mark_approval_alert_decided(state, user_id, &response.id, "approved").await?;

    state
        .automation_service
        .create_alert(
            user_id,
            None,
            "MCP request approved",
            &format!("{} was approved and executed.", tool_label(&response.tool)),
            "success",
            json!({
                "type": "mcp_approval_approved",
                "approval_id": response.id.clone(),
                "tool": response.tool.clone(),
                "result": result.clone()
            }),
        )
        .await?;

    Ok(McpApprovalDecisionResponse {
        approval: response,
        result: Some(result),
    })
}

pub async fn reject_request(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
) -> Result<McpApprovalDecisionResponse, AppError> {
    let approval = find_pending_approval(state, user_id, approval_id).await?;
    let now = Utc::now();
    let result = json!({ "rejected": true });
    let mut active = approval.into_active_model();
    active.status = Set("rejected".to_owned());
    active.result = Set(Some(result.clone()));
    active.updated_at = Set(now.into());
    active.decided_at = Set(Some(now.into()));
    let approval = active.update(&state.db).await?;
    let response = approval_response(approval);
    mark_approval_alert_decided(state, user_id, &response.id, "rejected").await?;

    state
        .automation_service
        .create_alert(
            user_id,
            None,
            "MCP request rejected",
            &format!("{} was rejected.", tool_label(&response.tool)),
            "info",
            json!({
                "type": "mcp_approval_rejected",
                "approval_id": response.id.clone(),
                "tool": response.tool.clone()
            }),
        )
        .await?;

    Ok(McpApprovalDecisionResponse {
        approval: response,
        result: Some(result),
    })
}

async fn create_approval_request(
    state: &AppState,
    user_id: &str,
    tool: &str,
    payload: Value,
) -> Result<Value, AppError> {
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);
    let approval = mcp_approval_request::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        user_id: Set(user_id.to_owned()),
        tool: Set(tool.to_owned()),
        status: Set("pending".to_owned()),
        payload: Set(payload.clone()),
        result: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        decided_at: Set(None),
        expires_at: Set(expires_at.into()),
    }
    .insert(&state.db)
    .await?;
    let response = approval_response(approval);

    state
        .automation_service
        .create_alert(
            user_id,
            None,
            "MCP approval required",
            &format!(
                "{} needs your approval before Uptions makes changes.",
                tool_label(tool)
            ),
            "pending",
            json!({
                "type": "mcp_approval_requested",
                "approval_id": response.id.clone(),
                "tool": tool,
                "action_label": tool_label(tool),
                "payload": payload
            }),
        )
        .await?;

    Ok(tool_result(json!({
        "approval_required": true,
        "approval_id": response.id,
        "status": response.status,
        "expires_at": response.expires_at
    })))
}

async fn execute_approved_tool(
    state: &AppState,
    user_id: &str,
    tool: &str,
    payload: Value,
) -> Result<Value, AppError> {
    match tool {
        "create_automation" => {
            let args: AutomationToolPayload = parse_params(payload, "Invalid automation payload")?;
            let automation = state
                .automation_service
                .publish(user_id, publish_request(args), None)
                .await?;
            Ok(json!({ "automation": automation }))
        }
        "update_automation" => {
            let args: UpdateAutomationToolPayload =
                parse_params(payload, "Invalid automation update payload")?;
            let automation_id = args.automation_id.clone();
            let automation = state
                .automation_service
                .update(user_id, &automation_id, publish_request(args.into()))
                .await?;
            Ok(json!({ "automation": automation }))
        }
        "pause_automation" => {
            let args: AutomationIdPayload = parse_params(payload, "Invalid automation arguments")?;
            let automation = state
                .automation_service
                .set_status(user_id, &args.automation_id, AutomationStatus::Paused)
                .await?;
            Ok(json!({ "automation": automation }))
        }
        "resume_automation" => {
            let args: AutomationIdPayload = parse_params(payload, "Invalid automation arguments")?;
            let automation = state
                .automation_service
                .set_status(user_id, &args.automation_id, AutomationStatus::Active)
                .await?;
            Ok(json!({ "automation": automation }))
        }
        "delete_automation" => {
            let args: AutomationIdPayload = parse_params(payload, "Invalid automation arguments")?;
            state
                .automation_service
                .delete(user_id, &args.automation_id)
                .await?;
            Ok(json!({ "deleted": true, "automation_id": args.automation_id }))
        }
        _ => Err(AppError::BadRequest(
            "unsupported MCP approval tool".to_owned(),
        )),
    }
}

async fn find_owned_approval(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
) -> Result<mcp_approval_request::Model, AppError> {
    let approval_id = approval_id.trim();

    if approval_id.is_empty() {
        return Err(AppError::BadRequest("approval id is required".to_owned()));
    }

    mcp_approval_request::Entity::find_by_id(approval_id.to_owned())
        .filter(mcp_approval_request::Column::UserId.eq(user_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("MCP approval request not found".to_owned()))
}

async fn find_pending_approval(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
) -> Result<mcp_approval_request::Model, AppError> {
    let approval = find_owned_approval(state, user_id, approval_id).await?;

    if approval.status != "pending" {
        return Err(AppError::Conflict(
            "MCP approval request is already decided".to_owned(),
        ));
    }

    if approval.expires_at < Utc::now().fixed_offset() {
        let now = Utc::now();
        let mut active = approval.into_active_model();
        active.status = Set("expired".to_owned());
        active.updated_at = Set(now.into());
        active.decided_at = Set(Some(now.into()));
        active.update(&state.db).await?;
        return Err(AppError::Conflict(
            "MCP approval request has expired".to_owned(),
        ));
    }

    Ok(approval)
}

async fn mark_approval_alert_decided(
    state: &AppState,
    user_id: &str,
    approval_id: &str,
    status: &str,
) -> Result<(), AppError> {
    let alerts = automation_alert::Entity::find()
        .filter(automation_alert::Column::UserId.eq(user_id))
        .all(&state.db)
        .await?;

    for alert in alerts {
        let matches_approval = alert
            .meta
            .get("approval_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value == approval_id);
        let matches_type = alert
            .meta
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "mcp_approval_requested");

        if matches_approval && matches_type {
            let mut meta = alert.meta.clone();

            if let Some(object) = meta.as_object_mut() {
                object.insert(
                    "approval_status".to_owned(),
                    Value::String(status.to_owned()),
                );
            }

            let mut active = alert.into_active_model();
            active.status = Set(status.to_owned());
            active.meta = Set(meta);
            active.read_at = Set(Some(Utc::now().into()));
            active.update(&state.db).await?;
        }
    }

    Ok(())
}

fn approval_response(model: mcp_approval_request::Model) -> McpApprovalResponse {
    McpApprovalResponse {
        id: model.id,
        tool: model.tool,
        status: model.status,
        payload: model.payload,
        result: model.result,
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
        decided_at: model.decided_at.map(|value| value.to_rfc3339()),
        expires_at: model.expires_at.to_rfc3339(),
    }
}

fn tool_label(tool: &str) -> &'static str {
    match tool {
        "create_automation" => "Create automation",
        "update_automation" => "Update automation",
        "pause_automation" => "Pause automation",
        "resume_automation" => "Resume automation",
        "delete_automation" => "Delete automation",
        _ => "MCP action",
    }
}

async fn has_ready_provider_connection(
    state: &AppState,
    user_id: &str,
    provider: ProviderId,
) -> Result<bool, AppError> {
    let connection = venue_connection::Entity::find()
        .filter(venue_connection::Column::UserId.eq(user_id))
        .filter(venue_connection::Column::Provider.eq(provider.storage_value()))
        .filter(venue_connection::Column::Enabled.eq(true))
        .filter(venue_connection::Column::Status.eq("active"))
        .one(&state.db)
        .await?;

    Ok(connection.is_some())
}

fn publish_request(payload: AutomationToolPayload) -> PublishAutomationRequest {
    PublishAutomationRequest {
        provider: payload.provider,
        market: payload.market,
        title: payload.title,
        workflow: payload.workflow,
    }
}

fn test_run_request(payload: TestRunAutomationToolPayload) -> TestRunAutomationRequest {
    TestRunAutomationRequest {
        automation_id: payload.automation_id,
        provider: payload.provider,
        market: payload.market,
        title: payload.title,
        workflow: payload.workflow,
    }
}

impl From<UpdateAutomationToolPayload> for AutomationToolPayload {
    fn from(payload: UpdateAutomationToolPayload) -> Self {
        Self {
            provider: payload.provider,
            market: payload.market,
            title: payload.title,
            workflow: payload.workflow,
        }
    }
}

fn market_analysis(provider: ProviderId, market: MarketResponse) -> Value {
    let outcome_prices = market
        .outcomes
        .iter()
        .map(|outcome| outcome.price)
        .collect::<Vec<_>>();

    json!({
        "provider": provider,
        "venue": provider.route_value(),
        "venue_label": provider.label(),
        "title": market.title,
        "description": market.description,
        "outcomes": market.outcomes,
        "outcome_prices": outcome_prices,
        "volume": market.volume,
        "liquidity": market.liquidity,
        "last_trade_price": market.last_trade_price,
        "best_bid": market.best_bid,
        "best_ask": market.best_ask,
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
        "market": market
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
        "required": ["provider"],
        "additionalProperties": false,
        "properties": {
            "provider": provider_schema(),
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
        "required": ["provider", "market_id"],
        "additionalProperties": false,
        "properties": {
            "provider": provider_schema(),
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
        "required": ["provider", "title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "provider": provider_schema(),
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn update_automation_payload_schema() -> Value {
    json!({
        "type": "object",
        "required": ["automation_id", "provider", "title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "automation_id": { "type": "string" },
            "provider": provider_schema(),
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn test_run_payload_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider", "title", "market", "workflow"],
        "additionalProperties": false,
        "properties": {
            "automation_id": { "type": "string" },
            "provider": provider_schema(),
            "title": { "type": "string" },
            "market": market_schema(),
            "workflow": workflow_schema()
        }
    })
}

fn prepare_trade_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider", "market", "action", "outcome", "order_type", "amount"],
        "additionalProperties": false,
        "properties": {
            "provider": provider_schema(),
            "market": market_schema(),
            "action": { "type": "string", "enum": [WorkflowActionType::Buy, WorkflowActionType::Sell] },
            "outcome": { "type": "string", "enum": ["YES", "NO"] },
            "order_type": { "type": "string", "enum": ["MARKET", "LIMIT"] },
            "amount": { "type": "number", "exclusiveMinimum": 0 }
        }
    })
}

fn provider_schema() -> Value {
    json!({ "type": "string", "enum": ["POLYMARKET"] })
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
        AppError::BadRequest(_) | AppError::ProviderValidation { .. } => -32602,
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
