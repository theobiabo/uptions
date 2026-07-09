use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    automations::dto::{
        AutomationAlertResponse, AutomationProvider, AutomationResponse, AutomationStatus,
        AutomationStepKind, PublishAutomationRequest, TestRunAutomationRequest,
        TestRunAutomationResponse, WorkflowActionType, WorkflowPayload,
    },
    db::Db,
    entities::{automation, automation_alert, venue_connection},
    error::AppError,
    notifications::{dto::AutomationAlertStreamEvent, service::NotificationService},
};

#[derive(Clone)]
pub struct AutomationService {
    db: Db,
    notifications: NotificationService,
}

impl AutomationService {
    pub fn new(db: Db, notifications: NotificationService) -> Self {
        Self { db, notifications }
    }

    pub async fn list(&self, user_id: &str) -> Result<Vec<AutomationResponse>, AppError> {
        let automations = automation::Entity::find()
            .filter(automation::Column::UserId.eq(user_id))
            .order_by_desc(automation::Column::UpdatedAt)
            .all(&self.db)
            .await?;

        Ok(automations.into_iter().map(automation_response).collect())
    }

    pub async fn alerts(&self, user_id: &str) -> Result<Vec<AutomationAlertResponse>, AppError> {
        let alerts = automation_alert::Entity::find()
            .filter(automation_alert::Column::UserId.eq(user_id))
            .order_by_desc(automation_alert::Column::CreatedAt)
            .limit(20)
            .all(&self.db)
            .await?;

        Ok(alerts.into_iter().map(alert_response).collect())
    }

    pub async fn mark_alert_read(
        &self,
        user_id: &str,
        alert_id: &str,
    ) -> Result<AutomationAlertResponse, AppError> {
        let alert_id = clean_required(alert_id, "alert id is required")?;
        let model = automation_alert::Entity::find_by_id(alert_id)
            .filter(automation_alert::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("notification not found".to_owned()))?;

        if model.read_at.is_some() {
            return Ok(alert_response(model));
        }

        let mut active = model.into_active_model();
        active.read_at = Set(Some(Utc::now().into()));
        let model = active.update(&self.db).await?;

        Ok(alert_response(model))
    }

    pub async fn mark_alerts_read(&self, user_id: &str) -> Result<u64, AppError> {
        let alerts = automation_alert::Entity::find()
            .filter(automation_alert::Column::UserId.eq(user_id))
            .filter(automation_alert::Column::ReadAt.is_null())
            .all(&self.db)
            .await?;
        let updated = alerts.len() as u64;
        let read_at = Utc::now();

        for model in alerts {
            let mut active = model.into_active_model();
            active.read_at = Set(Some(read_at.into()));
            active.update(&self.db).await?;
        }

        Ok(updated)
    }

    pub async fn clear_alerts(&self, user_id: &str) -> Result<u64, AppError> {
        let alerts = automation_alert::Entity::find()
            .filter(automation_alert::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;
        let deleted = alerts.len() as u64;

        for model in alerts {
            model.into_active_model().delete(&self.db).await?;
        }

        Ok(deleted)
    }

    pub async fn get(
        &self,
        user_id: &str,
        automation_id: &str,
    ) -> Result<AutomationResponse, AppError> {
        let model = self.find_owned_automation(user_id, automation_id).await?;

        Ok(automation_response(model))
    }

    pub async fn update(
        &self,
        user_id: &str,
        automation_id: &str,
        payload: PublishAutomationRequest,
    ) -> Result<AutomationResponse, AppError> {
        validate_workflow(&payload.workflow)?;
        self.validate_provider_readiness(user_id, payload.provider, &payload.workflow)
            .await?;
        let existing = self.find_owned_automation(user_id, automation_id).await?;
        let title = clean_title(&payload.title)?;
        let market_id = clean_required(&payload.market.id, "market id is required")?;
        let market_title = clean_required(&payload.market.title, "market title is required")?;
        let workflow = serde_json::to_value(&payload.workflow)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let mut active = existing.into_active_model();
        active.title = Set(title);
        active.market_id = Set(Some(market_id));
        active.market_title = Set(Some(market_title));
        active.venue = Set(payload.provider.venue_id().to_owned());
        active.workflow = Set(workflow);
        active.status = Set("active".to_owned());
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&self.db).await?;
        let response = automation_response(model);
        self.create_alert(
            user_id,
            Some(response.id.clone()),
            "Automation updated",
            &format!("{} was updated and is ready to run.", response.title),
            "success",
            json!({
                "type": "automation_updated",
                "automation_id": response.id.clone(),
                "market_id": response.market_id.clone(),
                "venue": response.venue.clone()
            }),
        )
        .await?;

        Ok(response)
    }

    pub async fn set_status(
        &self,
        user_id: &str,
        automation_id: &str,
        status: AutomationStatus,
    ) -> Result<AutomationResponse, AppError> {
        let model = self.find_owned_automation(user_id, automation_id).await?;
        let mut active = model.into_active_model();
        active.status = Set(status.as_str().to_owned());
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&self.db).await?;
        let response = automation_response(model);
        let action = match status {
            AutomationStatus::Active => "resumed",
            AutomationStatus::Paused => "paused",
        };
        self.create_alert(
            user_id,
            Some(response.id.clone()),
            &format!("Automation {action}"),
            &format!("{} was {action}.", response.title),
            "info",
            json!({
                "type": format!("automation_{action}"),
                "automation_id": response.id.clone(),
                "market_id": response.market_id.clone(),
                "status": response.status.clone()
            }),
        )
        .await?;

        Ok(response)
    }

    pub async fn delete(&self, user_id: &str, automation_id: &str) -> Result<(), AppError> {
        let model = self.find_owned_automation(user_id, automation_id).await?;
        let deleted_id = model.id.clone();
        let deleted_title = model.title.clone();
        let market_id = model.market_id.clone();
        model.into_active_model().delete(&self.db).await?;
        self.create_alert(
            user_id,
            None,
            "Automation deleted",
            &format!("{deleted_title} was deleted."),
            "info",
            json!({
                "type": "automation_deleted",
                "automation_id": deleted_id,
                "market_id": market_id
            }),
        )
        .await?;

        Ok(())
    }

    pub async fn publish(
        &self,
        user_id: &str,
        payload: PublishAutomationRequest,
    ) -> Result<AutomationResponse, AppError> {
        validate_workflow(&payload.workflow)?;
        self.validate_provider_readiness(user_id, payload.provider, &payload.workflow)
            .await?;
        let title = clean_title(&payload.title)?;
        let market_id = clean_required(&payload.market.id, "market id is required")?;
        let market_title = clean_required(&payload.market.title, "market title is required")?;
        let venue = payload.provider.venue_id().to_owned();
        let workflow = serde_json::to_value(&payload.workflow)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let now = Utc::now();
        let model = automation::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            title: Set(title),
            market_id: Set(Some(market_id)),
            market_title: Set(Some(market_title)),
            venue: Set(venue),
            status: Set("active".to_owned()),
            workflow: Set(workflow),
            last_run_status: Set(None),
            last_run_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;
        let response = automation_response(model);
        self.create_alert(
            user_id,
            Some(response.id.clone()),
            "Automation published",
            &format!("{} was published and is ready to run.", response.title),
            "success",
            json!({
                "type": "automation_published",
                "automation_id": response.id.clone(),
                "market_id": response.market_id.clone(),
                "venue": response.venue.clone()
            }),
        )
        .await?;

        Ok(response)
    }

    pub async fn test_run(
        &self,
        user_id: &str,
        payload: TestRunAutomationRequest,
    ) -> Result<TestRunAutomationResponse, AppError> {
        validate_workflow(&payload.workflow)?;
        let checked_blocks = payload.workflow.steps.len();
        let automation_id = clean_optional(payload.automation_id);
        let title = clean_title(&payload.title)?;
        let message = format!(
            "Test run completed successfully for {title} with {checked_blocks} workflow steps."
        );
        let alert = self
            .create_alert(
                user_id,
                automation_id.clone(),
                "Test run completed",
                &message,
                "success",
                json!({
                    "market": payload.market,
                    "provider": payload.provider,
                    "checked_blocks": checked_blocks
                }),
            )
            .await?;

        if let Some(id) = automation_id {
            if let Ok(model) = self.find_owned_automation(user_id, &id).await {
                let mut active = model.into_active_model();
                active.last_run_status = Set(Some("success".to_owned()));
                active.last_run_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await?;
            }
        }

        Ok(TestRunAutomationResponse {
            status: "success".to_owned(),
            message,
            checked_blocks,
            alert,
        })
    }

    async fn find_owned_automation(
        &self,
        user_id: &str,
        automation_id: &str,
    ) -> Result<automation::Model, AppError> {
        let automation_id = clean_required(automation_id, "automation id is required")?;

        if let Some(model) = automation::Entity::find_by_id(automation_id.clone())
            .filter(automation::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
        {
            return Ok(model);
        }

        automation::Entity::find()
            .filter(automation::Column::UserId.eq(user_id))
            .filter(automation::Column::MarketId.eq(Some(automation_id)))
            .order_by_desc(automation::Column::UpdatedAt)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("automation not found".to_owned()))
    }

    async fn validate_provider_readiness(
        &self,
        user_id: &str,
        provider: AutomationProvider,
        workflow: &WorkflowPayload,
    ) -> Result<(), AppError> {
        if provider != AutomationProvider::Polymarket {
            return Err(AppError::BadRequest(
                "unsupported automation provider".to_owned(),
            ));
        }

        if !workflow
            .steps
            .iter()
            .any(|step| is_trade_action(step.action))
        {
            return Ok(());
        }

        let connection = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(user_id))
            .filter(venue_connection::Column::Venue.eq(provider.venue_id()))
            .filter(venue_connection::Column::Enabled.eq(true))
            .filter(venue_connection::Column::Status.eq("active"))
            .one(&self.db)
            .await?;

        if connection.is_none() {
            return Err(AppError::BadRequest(
                "connect Polymarket before publishing trade actions".to_owned(),
            ));
        }

        Ok(())
    }

    pub async fn create_alert(
        &self,
        user_id: &str,
        automation_id: Option<String>,
        title: &str,
        message: &str,
        status: &str,
        meta: Value,
    ) -> Result<AutomationAlertResponse, AppError> {
        let alert = automation_alert::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            automation_id: Set(automation_id),
            title: Set(title.to_owned()),
            message: Set(message.to_owned()),
            status: Set(status.to_owned()),
            meta: Set(meta),
            created_at: Set(Utc::now().into()),
            read_at: Set(None),
        }
        .insert(&self.db)
        .await?;
        let alert = alert_response(alert);
        self.notifications
            .publish_alert(user_id, stream_alert_response(&alert));

        Ok(alert)
    }
}

fn validate_workflow(workflow: &WorkflowPayload) -> Result<(), AppError> {
    if workflow.version != 1 {
        return Err(AppError::BadRequest(
            "unsupported workflow version".to_owned(),
        ));
    }

    if workflow.steps.is_empty() {
        return Err(AppError::BadRequest(
            "workflow must contain at least one step".to_owned(),
        ));
    }

    let mut ids = HashSet::new();

    for step in &workflow.steps {
        clean_required(&step.id, "workflow step id is required")?;

        if !ids.insert(step.id.as_str()) {
            return Err(AppError::BadRequest(
                "workflow contains duplicate step ids".to_owned(),
            ));
        }

        validate_action_kind(step.kind, step.action)?;
        validate_params(step.action, &step.params)?;
    }

    if !workflow
        .steps
        .iter()
        .any(|step| step.kind == AutomationStepKind::Trigger)
    {
        return Err(AppError::BadRequest(
            "workflow must contain at least one trigger".to_owned(),
        ));
    }

    if !workflow
        .steps
        .iter()
        .any(|step| step.kind == AutomationStepKind::Action)
    {
        return Err(AppError::BadRequest(
            "workflow must contain at least one action".to_owned(),
        ));
    }

    if workflow.steps.len() > 1 && workflow.connections.is_empty() {
        return Err(AppError::BadRequest(
            "connect workflow steps before publishing".to_owned(),
        ));
    }

    validate_connections(workflow, &ids)
}

fn validate_connections(workflow: &WorkflowPayload, ids: &HashSet<&str>) -> Result<(), AppError> {
    let mut pairs = HashSet::new();
    let steps = workflow
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<HashMap<_, _>>();

    for connection in &workflow.connections {
        if !ids.contains(connection.from.as_str()) || !ids.contains(connection.to.as_str()) {
            return Err(AppError::BadRequest(
                "workflow connection references a missing step".to_owned(),
            ));
        }

        if connection.from == connection.to {
            return Err(AppError::BadRequest(
                "workflow step cannot connect to itself".to_owned(),
            ));
        }

        let pair = format!("{}:{}", connection.from, connection.to);

        if !pairs.insert(pair) {
            return Err(AppError::BadRequest(
                "workflow contains duplicate connections".to_owned(),
            ));
        }

        let Some(source) = steps.get(connection.from.as_str()) else {
            return Err(AppError::BadRequest(
                "workflow connection references a missing step".to_owned(),
            ));
        };
        let Some(target) = steps.get(connection.to.as_str()) else {
            return Err(AppError::BadRequest(
                "workflow connection references a missing step".to_owned(),
            ));
        };

        if kind_order(source.kind) > kind_order(target.kind) {
            return Err(AppError::BadRequest(
                "workflow steps must flow from trigger to condition to action".to_owned(),
            ));
        }
    }

    if has_cycle(workflow) {
        return Err(AppError::BadRequest(
            "workflow cannot contain loops".to_owned(),
        ));
    }

    if workflow.steps.len() > 1 && has_disconnected_steps(workflow) {
        return Err(AppError::BadRequest(
            "connect all workflow steps into one executable path".to_owned(),
        ));
    }

    Ok(())
}

fn validate_action_kind(
    kind: AutomationStepKind,
    action: WorkflowActionType,
) -> Result<(), AppError> {
    let expected = match action {
        WorkflowActionType::TriggerPriceMoves
        | WorkflowActionType::TriggerVolumeMoves
        | WorkflowActionType::TriggerTimeCheck => AutomationStepKind::Trigger,
        WorkflowActionType::ConditionOutcomePriceAbove
        | WorkflowActionType::ConditionOutcomePriceBelow
        | WorkflowActionType::ConditionVolumeAbove => AutomationStepKind::Condition,
        WorkflowActionType::Buy | WorkflowActionType::Sell | WorkflowActionType::SendMessage => {
            AutomationStepKind::Action
        }
    };

    if kind != expected {
        return Err(AppError::BadRequest(format!(
            "workflow action {action:?} must use kind {expected:?}"
        )));
    }

    Ok(())
}

fn validate_params(action: WorkflowActionType, params: &Value) -> Result<(), AppError> {
    match action {
        WorkflowActionType::TriggerPriceMoves => {
            string_enum(params, "outcome", &["YES", "NO"])?;
        }
        WorkflowActionType::TriggerVolumeMoves => {
            positive_number(
                params,
                "minimum_change_percent",
                "minimum_change_percent must be positive",
            )?;
        }
        WorkflowActionType::TriggerTimeCheck => {
            non_empty_string(params, "interval", "interval is required")?;
        }
        WorkflowActionType::ConditionOutcomePriceAbove => {
            string_enum(params, "outcome", &["YES", "NO"])?;
            string_enum(params, "operator", &["ABOVE"])?;
            probability(params, "price")?;
        }
        WorkflowActionType::ConditionOutcomePriceBelow => {
            string_enum(params, "outcome", &["YES", "NO"])?;
            string_enum(params, "operator", &["BELOW"])?;
            probability(params, "price")?;
        }
        WorkflowActionType::ConditionVolumeAbove => {
            string_enum(params, "operator", &["ABOVE"])?;
            positive_number(params, "volume", "volume must be positive")?;
        }
        WorkflowActionType::Buy | WorkflowActionType::Sell => {
            string_enum(params, "outcome", &["YES", "NO"])?;
            string_enum(params, "order_type", &["MARKET", "LIMIT"])?;
            positive_number(params, "amount", "amount must be positive")?;
        }
        WorkflowActionType::SendMessage => {
            string_enum(params, "channel", &["IN_APP"])?;
            non_empty_string(params, "message", "message is required")?;
        }
    }

    Ok(())
}

fn has_cycle(workflow: &WorkflowPayload) -> bool {
    let mut graph = HashMap::<&str, Vec<&str>>::new();

    for step in &workflow.steps {
        graph.insert(step.id.as_str(), Vec::new());
    }

    for connection in &workflow.connections {
        graph
            .entry(connection.from.as_str())
            .or_default()
            .push(connection.to.as_str());
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    workflow
        .steps
        .iter()
        .any(|step| visit(step.id.as_str(), &graph, &mut visiting, &mut visited))
}

fn visit<'a>(
    id: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    if visiting.contains(id) {
        return true;
    }

    if visited.contains(id) {
        return false;
    }

    visiting.insert(id);

    for next in graph.get(id).map(Vec::as_slice).unwrap_or_default() {
        if visit(next, graph, visiting, visited) {
            return true;
        }
    }

    visiting.remove(id);
    visited.insert(id);
    false
}

fn has_disconnected_steps(workflow: &WorkflowPayload) -> bool {
    let mut connected = HashSet::new();

    for connection in &workflow.connections {
        connected.insert(connection.from.as_str());
        connected.insert(connection.to.as_str());
    }

    workflow
        .steps
        .iter()
        .any(|step| !connected.contains(step.id.as_str()))
}

fn is_trade_action(action: WorkflowActionType) -> bool {
    matches!(action, WorkflowActionType::Buy | WorkflowActionType::Sell)
}

fn kind_order(kind: AutomationStepKind) -> u8 {
    match kind {
        AutomationStepKind::Trigger => 1,
        AutomationStepKind::Condition => 2,
        AutomationStepKind::Action => 3,
    }
}

fn string_enum(params: &Value, key: &str, allowed: &[&str]) -> Result<(), AppError> {
    let Some(value) = params.get(key).and_then(Value::as_str) else {
        return Err(AppError::BadRequest(format!("{key} is required")));
    };

    if !allowed.contains(&value) {
        return Err(AppError::BadRequest(format!("{key} is invalid")));
    }

    Ok(())
}

fn non_empty_string(params: &Value, key: &str, message: &str) -> Result<(), AppError> {
    let Some(value) = params.get(key).and_then(Value::as_str) else {
        return Err(AppError::BadRequest(message.to_owned()));
    };

    if value.trim().is_empty() {
        return Err(AppError::BadRequest(message.to_owned()));
    }

    Ok(())
}

fn positive_number(params: &Value, key: &str, message: &str) -> Result<(), AppError> {
    let Some(value) = params.get(key).and_then(Value::as_f64) else {
        return Err(AppError::BadRequest(message.to_owned()));
    };

    if !value.is_finite() || value <= 0.0 {
        return Err(AppError::BadRequest(message.to_owned()));
    }

    Ok(())
}

fn probability(params: &Value, key: &str) -> Result<(), AppError> {
    let Some(value) = params.get(key).and_then(Value::as_f64) else {
        return Err(AppError::BadRequest(
            "price must be between 0 and 1".to_owned(),
        ));
    };

    if !value.is_finite() || value <= 0.0 || value >= 1.0 {
        return Err(AppError::BadRequest(
            "price must be between 0 and 1".to_owned(),
        ));
    }

    Ok(())
}

fn clean_title(value: &str) -> Result<String, AppError> {
    clean_required(value, "automation title is required")
}

fn clean_required(value: &str, message: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(message.to_owned()));
    }
    Ok(value.to_owned())
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn automation_response(model: automation::Model) -> AutomationResponse {
    AutomationResponse {
        id: model.id,
        title: model.title,
        market_id: model.market_id,
        market_title: model.market_title,
        venue: model.venue,
        status: model.status,
        workflow: model.workflow,
        last_run_status: model.last_run_status,
        last_run_at: model.last_run_at.map(|value| value.to_rfc3339()),
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
    }
}

fn alert_response(model: automation_alert::Model) -> AutomationAlertResponse {
    AutomationAlertResponse {
        id: model.id,
        automation_id: model.automation_id,
        title: model.title,
        message: model.message,
        status: model.status,
        meta: model.meta,
        created_at: model.created_at.to_rfc3339(),
        read_at: model.read_at.map(|value| value.to_rfc3339()),
    }
}

fn stream_alert_response(alert: &AutomationAlertResponse) -> AutomationAlertStreamEvent {
    AutomationAlertStreamEvent {
        automation_id: alert.automation_id.clone(),
        created_at: alert.created_at.clone(),
        event_type: "automation_alert".to_owned(),
        id: alert.id.clone(),
        message: alert.message.clone(),
        meta: alert.meta.clone(),
        read_at: alert.read_at.clone(),
        status: alert.status.clone(),
        title: alert.title.clone(),
    }
}
