use crate::{
    db::Db,
    entities::{automation, automation_alert, user, waitlist},
    error::AppError,
    libs::resend_client::send_email,
    providers::types::ProviderId,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

pub struct JoinWaitlistStruct {
    pub email: String,
}

#[derive(Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn set_preferred_trading_provider(
        &self,
        user_id: &str,
        provider: ProviderId,
    ) -> Result<ProviderId, AppError> {
        let model = user::Entity::find_by_id(user_id.to_owned())
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let previous = ProviderId::from_storage(&model.preferred_trading_provider);
        if previous == Some(provider) {
            return Ok(provider);
        }
        let txn = self.db.begin().await?;
        let mut active = model.into_active_model();
        active.preferred_trading_provider = Set(provider.storage_value().to_owned());
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        let paused = automation::Entity::update_many()
            .set(automation::ActiveModel {
                status: Set("paused".to_owned()),
                last_run_status: Set(Some("action_required_provider_changed".to_owned())),
                updated_at: Set(Utc::now().into()),
                ..Default::default()
            })
            .filter(automation::Column::UserId.eq(user_id))
            .filter(automation::Column::Status.eq("active"))
            .filter(automation::Column::Provider.ne(provider.storage_value()))
            .exec(&txn)
            .await?;
        if paused.rows_affected > 0 {
            automation_alert::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                user_id: Set(user_id.to_owned()),
                automation_id: Set(None),
                title: Set("Automations paused".to_owned()),
                message: Set(format!(
                    "{} automation(s) use your previous provider and were paused for review.",
                    paused.rows_affected
                )),
                status: Set("warning".to_owned()),
                meta: Set(json!({
                    "type": "automation_provider_changed",
                    "selected_provider": provider,
                    "paused_count": paused.rows_affected
                })),
                created_at: Set(Utc::now().into()),
                read_at: Set(None),
            }
            .insert(&txn)
            .await?;
        }
        txn.commit().await?;
        Ok(provider)
    }

    pub async fn join_waitlist(&self, payload: JoinWaitlistStruct) -> Result<(), AppError> {
        let email = payload.email.trim().to_lowercase();

        if email.is_empty() {
            return Err(AppError::BadRequest("email is required".to_owned()));
        }

        let existing = waitlist::Entity::find()
            .filter(waitlist::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict(
                "user already exists on the waitlist".to_owned(),
            ));
        }

        waitlist::ActiveModel {
            email: Set(email.clone()),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        let subject = "You are on the Uptions waitlist";
        let html_body = waitlist_email_template(&email);

        if let Err(error) = send_email(&email, subject, &html_body).await {
            tracing::error!(email = %email, error = %error, "failed to send waitlist email");
        }

        Ok(())
    }
}

fn waitlist_email_template(email: &str) -> String {
    let escaped_email = escape_html(email);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Uptions waitlist</title>
</head>
<body style="margin:0; padding:0; background:#f7f7f3; color:#111111; font-family:Outfit, Arial, sans-serif;">
  <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f7f7f3; margin:0; padding:32px 16px;">
    <tr>
      <td align="center">
        <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px; background:#ffffff; border:1px solid rgba(17,17,17,0.10);">
          <tr>
            <td style="padding:28px 28px 0;">
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0">
                <tr>
                  <td style="font-size:20px; line-height:1; font-weight:800; letter-spacing:0; color:#111111;">
                    Uptions<span style="color:#ff4f00;">.</span>
                  </td>
                  <td align="right">
                    <span style="display:inline-block; padding:7px 10px; border:1px solid rgba(17,17,17,0.10); color:rgba(17,17,17,0.58); font-size:12px; line-height:1; font-weight:700;">Waitlist</span>
                  </td>
                </tr>
              </table>
            </td>
          </tr>
          <tr>
            <td style="padding:44px 28px 20px;">
              <h1 style="margin:0; color:#111111; font-size:38px; line-height:0.98; font-weight:800; letter-spacing:0;">You are on the list.</h1>
              <p style="margin:18px 0 0; color:rgba(17,17,17,0.64); font-size:16px; line-height:1.65;">Thanks for joining Uptions. We saved <strong style="color:#111111;">{escaped_email}</strong> and will send product access updates to this address.</p>
            </td>
          </tr>
          <tr>
            <td style="padding:8px 28px 28px;">
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="border:1px solid rgba(17,17,17,0.10); background:#ffffff;">
                <tr>
                  <td style="padding:18px 18px 16px; border-bottom:1px solid rgba(17,17,17,0.08);">
                    <p style="margin:0 0 6px; color:#ff4f00; font-size:11px; line-height:1; font-weight:800; text-transform:uppercase;">Next</p>
                    <p style="margin:0; color:#111111; font-size:15px; line-height:1.55; font-weight:700;">Early access and product notes</p>
                    <p style="margin:6px 0 0; color:rgba(17,17,17,0.58); font-size:14px; line-height:1.55;">We will share updates as the automation dashboard opens up.</p>
                  </td>
                </tr>
                <tr>
                  <td style="padding:16px 18px;">
                    <p style="margin:0 0 6px; color:#00a85a; font-size:11px; line-height:1; font-weight:800; text-transform:uppercase;">Status</p>
                    <p style="margin:0; color:#111111; font-size:15px; line-height:1.55; font-weight:700;">Confirmed</p>
                  </td>
                </tr>
              </table>
            </td>
          </tr>
          <tr>
            <td style="padding:0 28px 30px;">
              <p style="margin:0; color:rgba(17,17,17,0.46); font-size:12px; line-height:1.6;">Uptions helps automate prediction market strategies with wallet identity, connected markets, and workflow automation.</p>
            </td>
          </tr>
        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
