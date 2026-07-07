use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    automations::dto::{
        AutomationAlertResponse, AutomationResponse, PublishAutomationRequest,
        TestRunAutomationRequest, TestRunAutomationResponse,
    },
    db::Db,
    entities::{automation, automation_alert},
    error::AppError,
};

#[derive(Clone)]
pub struct AutomationService {
    db: Db,
}

impl AutomationService {
    pub fn new(db: Db) -> Self {
        Self { db }
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

    pub async fn publish(
        &self,
        user_id: &str,
        payload: PublishAutomationRequest,
    ) -> Result<AutomationResponse, AppError> {
        validate_workflow(&payload.workflow)?;
        let title = clean_title(&payload.title)?;
        let workflow = serde_json::to_value(payload.workflow)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let now = Utc::now();
        let model = automation::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            title: Set(title),
            market_id: Set(clean_optional(payload.market_id)),
            market_title: Set(clean_optional(payload.market_title)),
            venue: Set(clean_required(&payload.venue, "venue is required")?),
            status: Set("active".to_owned()),
            workflow: Set(workflow),
            last_run_status: Set(None),
            last_run_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        Ok(automation_response(model))
    }

    pub async fn test_run(
        &self,
        user_id: &str,
        payload: TestRunAutomationRequest,
    ) -> Result<TestRunAutomationResponse, AppError> {
        validate_workflow(&payload.workflow)?;
        let checked_blocks = payload.workflow.nodes.len();
        let automation_id = clean_optional(payload.automation_id);
        let title = clean_title(&payload.title)?;
        let message = format!(
            "Test run completed successfully for {title} with {checked_blocks} workflow blocks."
        );
        let alert = self.create_alert(user_id, automation_id.clone(), "Test run completed", &message, "success", json!({ "market_id": payload.market_id, "market_title": payload.market_title, "venue": payload.venue, "checked_blocks": checked_blocks })).await?;

        if let Some(id) = automation_id {
            if let Some(model) = automation::Entity::find_by_id(id)
                .filter(automation::Column::UserId.eq(user_id))
                .one(&self.db)
                .await?
            {
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

    async fn create_alert(
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
        }
        .insert(&self.db)
        .await?;

        Ok(alert_response(alert))
    }
}

fn validate_workflow(workflow: &crate::automations::dto::WorkflowPayload) -> Result<(), AppError> {
    if workflow.nodes.is_empty() {
        return Err(AppError::BadRequest(
            "workflow must contain at least one block".to_owned(),
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
    }
}
