use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use k256::{
    EncodedPoint,
    ecdsa::{RecoveryId, Signature, VerifyingKey},
};
use rand_core::{OsRng, RngCore};
use sha3::{Digest, Keccak256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    auth::dto::{
        AuthSessionResponse, AuthUserResponse, ConnectPolymarketRequest, CreateChallengeResponse,
        ForgotPasswordRequest, LoginRequest, ResetPasswordRequest, SignupRequest,
        VenueConnectionResponse, VerifyChallengeResponse,
    },
    db::Db,
    entities::{auth_method, user, user_session, venue_connection},
    error::AppError,
    libs::resend_client::send_email,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set,
    sea_query::OnConflict,
};
use serde_json::{Value, json};

const CHALLENGE_TTL_SECONDS: u64 = 300;
const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24 * 30;
const EMAIL_VERIFICATION_TTL_SECONDS: u64 = 60 * 60 * 24;
const PASSWORD_RESET_TTL_SECONDS: u64 = 60 * 60;
const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Clone)]
pub struct AuthService {
    challenges: Arc<RwLock<HashMap<String, ChallengeRecord>>>,
    app_base_url: String,
    credential_encryption_key: [u8; 32],
    db: Db,
}

#[derive(Clone)]
struct ChallengeRecord {
    wallet_address: String,
    message: String,
    expires_at: u64,
}

#[derive(Clone)]
struct SessionRecord {
    user_id: String,
}

impl AuthService {
    pub fn new(db: Db, credential_encryption_key: String, app_base_url: String) -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            app_base_url: app_base_url.trim_end_matches('/').to_owned(),
            credential_encryption_key: parse_encryption_key(&credential_encryption_key)
                .expect("CREDENTIAL_ENCRYPTION_KEY must resolve to 32 bytes"),
            db,
        }
    }

    pub async fn create_challenge(
        &self,
        wallet_address: &str,
    ) -> Result<CreateChallengeResponse, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let nonce = Uuid::new_v4().to_string();
        let expires_at = unix_timestamp() + CHALLENGE_TTL_SECONDS;
        let message = format!("Sign in to Uptions\nAddress: {wallet_address}\nNonce: {nonce}");

        let record = ChallengeRecord {
            wallet_address: wallet_address.clone(),
            message: message.clone(),
            expires_at,
        };

        self.challenges
            .write()
            .await
            .insert(wallet_address.clone(), record);

        Ok(CreateChallengeResponse {
            wallet_address,
            nonce,
            message,
            expires_at,
        })
    }

    pub async fn signup(&self, payload: SignupRequest) -> Result<AuthUserResponse, AppError> {
        let email = normalize_email(&payload.email)?;
        validate_password(&payload.password)?;

        let existing = user::Entity::find()
            .filter(user::Column::Email.eq(Some(email.clone())))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict("email is already registered".to_owned()));
        }

        let password_hash = hash_password(&payload.password)?;
        let verification_token = generate_auth_token();
        let verification_expires_at =
            timestamp_after(Duration::from_secs(EMAIL_VERIFICATION_TTL_SECONDS));
        let user_id = Uuid::new_v4().to_string();
        let user = user::ActiveModel {
            id: Set(user_id.clone()),
            email: Set(Some(email.clone())),
            password_hash: Set(Some(password_hash)),
            email_verification_token_hash: Set(Some(hash_access_token(&verification_token))),
            email_verification_expires_at: Set(Some(verification_expires_at.into())),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        self.ensure_email_auth_method(&user_id, &email).await?;
        self.send_verification_email(&email, &verification_token)
            .await;
        self.auth_user_response(&user).await
    }

    pub async fn login(&self, payload: LoginRequest) -> Result<AuthSessionResponse, AppError> {
        let email = normalize_email(&payload.email)?;
        let user = user::Entity::find()
            .filter(user::Column::Email.eq(Some(email.clone())))
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        let Some(password_hash) = &user.password_hash else {
            return Err(AppError::Unauthorized);
        };

        if user.email.is_some() && user.email_verified_at.is_none() {
            return Err(AppError::Unauthorized);
        }

        if !verify_password(&payload.password, password_hash)? {
            return Err(AppError::Unauthorized);
        }

        self.issue_session(user).await
    }

    pub async fn verify_email(&self, token: &str) -> Result<AuthSessionResponse, AppError> {
        let token_hash = hash_access_token(normalize_token(token)?);
        let user = user::Entity::find()
            .filter(user::Column::EmailVerificationTokenHash.eq(Some(token_hash)))
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("verification link is invalid".to_owned()))?;

        let expires_at = user
            .email_verification_expires_at
            .ok_or_else(|| AppError::BadRequest("verification link is invalid".to_owned()))?;

        if expires_at.with_timezone(&Utc) < Utc::now() {
            return Err(AppError::BadRequest(
                "verification link has expired".to_owned(),
            ));
        }

        let mut active = user.into_active_model();
        active.email_verified_at = Set(Some(Utc::now().into()));
        active.email_verification_token_hash = Set(None);
        active.email_verification_expires_at = Set(None);
        let user = active.update(&self.db).await?;

        self.issue_session(user).await
    }

    pub async fn forgot_password(&self, payload: ForgotPasswordRequest) -> Result<(), AppError> {
        let email = normalize_email(&payload.email)?;
        let Some(user) = user::Entity::find()
            .filter(user::Column::Email.eq(Some(email.clone())))
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        if user.password_hash.is_none() {
            return Ok(());
        }

        let reset_token = generate_auth_token();
        let reset_expires_at = timestamp_after(Duration::from_secs(PASSWORD_RESET_TTL_SECONDS));
        let mut active = user.into_active_model();
        active.password_reset_token_hash = Set(Some(hash_access_token(&reset_token)));
        active.password_reset_expires_at = Set(Some(reset_expires_at.into()));
        active.update(&self.db).await?;

        self.send_password_reset_email(&email, &reset_token).await;

        Ok(())
    }

    pub async fn reset_password(
        &self,
        payload: ResetPasswordRequest,
    ) -> Result<AuthSessionResponse, AppError> {
        let token_hash = hash_access_token(normalize_token(&payload.token)?);
        validate_password(&payload.password)?;

        let user = user::Entity::find()
            .filter(user::Column::PasswordResetTokenHash.eq(Some(token_hash)))
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("reset link is invalid".to_owned()))?;

        let expires_at = user
            .password_reset_expires_at
            .ok_or_else(|| AppError::BadRequest("reset link is invalid".to_owned()))?;

        if expires_at.with_timezone(&Utc) < Utc::now() {
            return Err(AppError::BadRequest("reset link has expired".to_owned()));
        }

        let mut active = user.into_active_model();
        active.password_hash = Set(Some(hash_password(&payload.password)?));
        active.password_reset_token_hash = Set(None);
        active.password_reset_expires_at = Set(None);
        active.email_verified_at = Set(Some(Utc::now().into()));
        let user = active.update(&self.db).await?;

        self.issue_session(user).await
    }

    pub async fn verify_challenge(
        &self,
        wallet_address: &str,
        signature: &str,
    ) -> Result<VerifyChallengeResponse, AppError> {
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let challenge = self
            .challenges
            .write()
            .await
            .remove(&wallet_address)
            .ok_or_else(|| AppError::BadRequest("challenge not found for wallet".to_owned()))?;

        if challenge.expires_at < unix_timestamp() {
            return Err(AppError::BadRequest("challenge expired".to_owned()));
        }

        if challenge.wallet_address != wallet_address {
            return Err(AppError::BadRequest("challenge wallet mismatch".to_owned()));
        }

        let recovered_address = recover_wallet_address(&challenge.message, signature)?;
        if recovered_address != wallet_address {
            return Err(AppError::Unauthorized);
        }

        let user = self.ensure_wallet_user(&wallet_address).await?;
        let session = self.create_session(&user.id).await?;

        Ok(VerifyChallengeResponse {
            access_token: session.access_token,
            token_type: "Bearer".to_owned(),
            user: self.auth_user_response(&user).await?,
        })
    }

    pub async fn current_user(&self, access_token: &str) -> Result<AuthUserResponse, AppError> {
        let session = self.session(access_token).await?;
        let user = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        Ok(self.auth_user_response(&user).await?)
    }

    pub async fn connect_polymarket(
        &self,
        access_token: &str,
        payload: ConnectPolymarketRequest,
    ) -> Result<VenueConnectionResponse, AppError> {
        let session = self.session(access_token).await?;
        let user = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if payload.api_key.trim().is_empty()
            || payload.secret.trim().is_empty()
            || payload.passphrase.trim().is_empty()
        {
            return Err(AppError::BadRequest(
                "polymarket credentials are required".to_owned(),
            ));
        }

        let account_identifier = match payload.account_identifier {
            Some(address) => normalize_wallet_address(&address)?,
            None => user.primary_wallet_address.clone().ok_or_else(|| {
                AppError::BadRequest(
                    "account_identifier is required for email-authenticated users".to_owned(),
                )
            })?,
        };
        let funder = payload
            .funder
            .map(|address| normalize_wallet_address(&address))
            .transpose()?;
        let signature_type = polymarket_signature_type(payload.signature_type)?;
        let limits = payload.limits.unwrap_or_else(|| json!({}));
        let permissions = payload.permissions.unwrap_or_else(default_permissions);
        let credential_config = json!({
            "apiKey": payload.api_key,
            "secret": payload.secret,
            "passphrase": payload.passphrase,
            "funder": funder,
            "signatureType": signature_type
        });
        let config = encrypt_json(&self.credential_encryption_key, &credential_config)?;

        let existing = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(&session.user_id))
            .filter(venue_connection::Column::Venue.eq("polymarket"))
            .one(&self.db)
            .await?;

        let connection = match existing {
            Some(model) => {
                let mut active = model.into_active_model();
                active.account_identifier = Set(account_identifier);
                active.config = Set(config);
                active.enabled = Set(true);
                active.limits = Set(limits);
                active.auth_type = Set("api_key".to_owned());
                active.permissions = Set(permissions);
                active.status = Set("active".to_owned());
                active.update(&self.db).await?
            }
            None => {
                venue_connection::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    user_id: Set(session.user_id),
                    venue: Set("polymarket".to_owned()),
                    account_identifier: Set(account_identifier),
                    auth_type: Set("api_key".to_owned()),
                    config: Set(config),
                    enabled: Set(true),
                    limits: Set(limits),
                    permissions: Set(permissions),
                    status: Set("active".to_owned()),
                    ..Default::default()
                }
                .insert(&self.db)
                .await?
            }
        };

        Ok(venue_connection_response(connection))
    }

    async fn session(&self, access_token: &str) -> Result<SessionRecord, AppError> {
        let token_hash = hash_access_token(access_token);
        let session = user_session::Entity::find()
            .filter(user_session::Column::TokenHash.eq(token_hash))
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if session.expires_at.with_timezone(&Utc) < Utc::now() {
            return Err(AppError::Unauthorized);
        }

        Ok(SessionRecord {
            user_id: session.user_id,
        })
    }

    async fn ensure_wallet_user(&self, wallet_address: &str) -> Result<user::Model, AppError> {
        if let Some(model) = user::Entity::find()
            .filter(user::Column::PrimaryWalletAddress.eq(Some(wallet_address.to_owned())))
            .one(&self.db)
            .await?
        {
            self.ensure_wallet_auth_method(&model.id, wallet_address)
                .await?;
            return Ok(model);
        }

        let user_id = Uuid::new_v4().to_string();
        let model = user::ActiveModel {
            id: Set(user_id.clone()),
            primary_wallet_address: Set(Some(wallet_address.to_owned())),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        self.ensure_wallet_auth_method(&user_id, wallet_address)
            .await?;

        Ok(model)
    }

    async fn ensure_wallet_auth_method(
        &self,
        user_id: &str,
        wallet_address: &str,
    ) -> Result<(), AppError> {
        auth_method::Entity::insert(auth_method::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            method_type: Set("wallet".to_owned()),
            external_id: Set(wallet_address.to_owned()),
            meta: Set(json!({})),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                auth_method::Column::MethodType,
                auth_method::Column::ExternalId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(&self.db)
        .await?;

        Ok(())
    }

    async fn ensure_email_auth_method(&self, user_id: &str, email: &str) -> Result<(), AppError> {
        auth_method::Entity::insert(auth_method::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            method_type: Set("email".to_owned()),
            external_id: Set(email.to_owned()),
            meta: Set(json!({})),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                auth_method::Column::MethodType,
                auth_method::Column::ExternalId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(&self.db)
        .await?;

        Ok(())
    }

    async fn issue_session(&self, user: user::Model) -> Result<AuthSessionResponse, AppError> {
        let session = self.create_session(&user.id).await?;

        Ok(AuthSessionResponse {
            access_token: session.access_token,
            token_type: "Bearer".to_owned(),
            expires_at: session.expires_at,
            user: self.auth_user_response(&user).await?,
        })
    }

    async fn create_session(&self, user_id: &str) -> Result<CreatedSession, AppError> {
        let access_token = Uuid::new_v4().to_string();
        let expires_at_system = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECONDS);
        let expires_at: DateTime<Utc> = expires_at_system.into();
        let expires_at_unix = expires_at.timestamp();

        user_session::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            token_hash: Set(hash_access_token(&access_token)),
            expires_at: Set(expires_at.into()),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        Ok(CreatedSession {
            access_token,
            expires_at: expires_at_unix,
        })
    }

    async fn send_verification_email(&self, email: &str, token: &str) {
        let subject = "Verify your Uptions account";
        let verify_url = format!("{}/?verify_email={token}", self.app_base_url);
        let html_body = verification_email_template(email, &verify_url);

        if let Err(error) = send_email(email, subject, &html_body).await {
            tracing::error!(email = %email, error = %error, "failed to send verification email");
        }
    }

    async fn send_password_reset_email(&self, email: &str, token: &str) {
        let subject = "Reset your Uptions password";
        let reset_url = format!("{}/?reset_password={token}", self.app_base_url);
        let html_body = password_reset_email_template(email, &reset_url);

        if let Err(error) = send_email(email, subject, &html_body).await {
            tracing::error!(email = %email, error = %error, "failed to send password reset email");
        }
    }

    async fn auth_user_response(&self, user: &user::Model) -> Result<AuthUserResponse, AppError> {
        let venue_connections = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(&user.id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(venue_connection_response)
            .collect();

        Ok(AuthUserResponse {
            id: user.id.clone(),
            primary_wallet_address: user.primary_wallet_address.clone(),
            wallet_address: user.primary_wallet_address.clone(),
            email: user.email.clone(),
            email_verified: user.email.is_none() || user.email_verified_at.is_some(),
            venue_connections,
        })
    }
}

struct CreatedSession {
    access_token: String,
    expires_at: i64,
}

fn venue_connection_response(model: venue_connection::Model) -> VenueConnectionResponse {
    VenueConnectionResponse {
        id: model.id,
        venue: model.venue,
        auth_type: model.auth_type,
        account_identifier: model.account_identifier,
        enabled: model.enabled,
        limits: redact_limits(model.limits),
        permissions: model.permissions,
        status: model.status,
    }
}

fn redact_limits(limits: Value) -> Value {
    limits
}

fn polymarket_signature_type(signature_type: Option<i32>) -> Result<i32, AppError> {
    let signature_type = signature_type.unwrap_or(3);

    if (0..=3).contains(&signature_type) {
        Ok(signature_type)
    } else {
        Err(AppError::BadRequest(
            "invalid polymarket signature type".to_owned(),
        ))
    }
}

fn normalize_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();

    if email.is_empty() || !email.contains('@') || email.len() > 255 {
        return Err(AppError::BadRequest("valid email is required".to_owned()));
    }

    Ok(email)
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters"
        )));
    }

    Ok(())
}

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::BadRequest(error.to_string()))
}

fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| AppError::Unauthorized)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn hash_access_token(access_token: &str) -> String {
    encode_hex(&keccak256(access_token.as_bytes()))
}

fn generate_auth_token() -> String {
    Uuid::new_v4().to_string()
}

fn normalize_token(token: &str) -> Result<&str, AppError> {
    let token = token.trim();

    if token.is_empty() || token.len() > 128 {
        return Err(AppError::BadRequest("valid token is required".to_owned()));
    }

    Ok(token)
}

fn timestamp_after(duration: Duration) -> DateTime<Utc> {
    (SystemTime::now() + duration).into()
}

fn encrypt_json(key: &[u8; 32], value: &Value) -> Result<Value, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::DatabaseError("invalid encryption key".to_owned()))?;
    let mut nonce_bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext =
        serde_json::to_vec(value).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|_| AppError::DatabaseError("failed to encrypt credentials".to_owned()))?;

    Ok(json!({
        "encrypted": true,
        "cipher": "AES-256-GCM",
        "nonce": STANDARD.encode(nonce_bytes),
        "payload": STANDARD.encode(ciphertext)
    }))
}

fn parse_encryption_key(value: &str) -> Result<[u8; 32], AppError> {
    let trimmed = value.trim();
    let decoded = if trimmed.len() == 64
        && trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        decode_hex(trimmed)
            .map_err(|_| AppError::BadRequest("invalid encryption key".to_owned()))?
    } else {
        STANDARD
            .decode(trimmed)
            .unwrap_or_else(|_| trimmed.as_bytes().to_vec())
    };

    if decoded.len() != 32 {
        return Err(AppError::BadRequest(
            "credential encryption key must be 32 bytes".to_owned(),
        ));
    }

    let mut key = [0_u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn default_permissions() -> Value {
    json!({
        "read": true,
        "trade": false,
        "automation": false
    })
}

fn verification_email_template(email: &str, verify_url: &str) -> String {
    let escaped_email = escape_html(email);
    let escaped_verify_url = escape_html(verify_url);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Verify your Uptions account</title>
</head>
<body style="margin:0; padding:0; background:#f5f5f1; color:#111111; font-family:Arial, sans-serif;">
  <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f5f1; margin:0; padding:32px 16px;">
    <tr>
      <td align="center">
        <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px; background:#ffffff; border:1px solid rgba(17,17,17,0.10);">
          <tr>
            <td style="padding:28px 28px 0;">
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0">
                <tr>
                  <td style="font-size:20px; line-height:1; font-weight:800; color:#111111;">Uptions<span style="color:#ff4f00;">.</span></td>
                  <td align="right"><span style="display:inline-block; padding:7px 10px; border:1px solid rgba(17,17,17,0.10); color:rgba(17,17,17,0.58); font-size:12px; line-height:1; font-weight:700;">Verify email</span></td>
                </tr>
              </table>
            </td>
          </tr>
          <tr>
            <td style="padding:42px 28px 20px;">
              <h1 style="margin:0; color:#111111; font-size:34px; line-height:1.05; font-weight:800;">Verify your email.</h1>
              <p style="margin:18px 0 0; color:rgba(17,17,17,0.66); font-size:16px; line-height:1.65;">Confirm <strong style="color:#111111;">{escaped_email}</strong> to finish creating your Uptions account.</p>
            </td>
          </tr>
          <tr>
            <td style="padding:8px 28px 30px;">
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="border:1px solid rgba(17,17,17,0.10); background:#ffffff;">
                <tr>
                  <td style="padding:18px;">
                    <p style="margin:0 0 6px; color:#ff4f00; font-size:11px; line-height:1; font-weight:800; text-transform:uppercase;">Next step</p>
                    <p style="margin:0; color:#111111; font-size:15px; line-height:1.55; font-weight:700;">This link expires in 24 hours.</p>
                    <p style="margin:18px 0 0;"><a href="{escaped_verify_url}" style="display:inline-block; background:#ff4f00; color:#ffffff; padding:12px 16px; text-decoration:none; font-size:14px; font-weight:800;">Verify account</a></p>
                  </td>
                </tr>
              </table>
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

fn password_reset_email_template(email: &str, reset_url: &str) -> String {
    let escaped_email = escape_html(email);
    let escaped_reset_url = escape_html(reset_url);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Reset your Uptions password</title>
</head>
<body style="margin:0; padding:0; background:#f5f5f1; color:#111111; font-family:Arial, sans-serif;">
  <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f5f1; margin:0; padding:32px 16px;">
    <tr>
      <td align="center">
        <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px; background:#ffffff; border:1px solid rgba(17,17,17,0.10);">
          <tr>
            <td style="padding:28px 28px 0;">
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0">
                <tr>
                  <td style="font-size:20px; line-height:1; font-weight:800; color:#111111;">Uptions<span style="color:#ff4f00;">.</span></td>
                  <td align="right"><span style="display:inline-block; padding:7px 10px; border:1px solid rgba(17,17,17,0.10); color:rgba(17,17,17,0.58); font-size:12px; line-height:1; font-weight:700;">Password reset</span></td>
                </tr>
              </table>
            </td>
          </tr>
          <tr>
            <td style="padding:42px 28px 20px;">
              <h1 style="margin:0; color:#111111; font-size:34px; line-height:1.05; font-weight:800;">Reset your password.</h1>
              <p style="margin:18px 0 0; color:rgba(17,17,17,0.66); font-size:16px; line-height:1.65;">Use this link to set a new password for <strong style="color:#111111;">{escaped_email}</strong>. It expires in 1 hour.</p>
            </td>
          </tr>
          <tr>
            <td style="padding:8px 28px 30px;">
              <p style="margin:0;"><a href="{escaped_reset_url}" style="display:inline-block; background:#ff4f00; color:#ffffff; padding:12px 16px; text-decoration:none; font-size:14px; font-weight:800;">Reset password</a></p>
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

fn recover_wallet_address(message: &str, signature: &str) -> Result<String, AppError> {
    let signature_bytes = decode_hex(signature).map_err(|_| AppError::Unauthorized)?;
    if signature_bytes.len() != 65 {
        return Err(AppError::Unauthorized);
    }

    let signature =
        Signature::try_from(&signature_bytes[..64]).map_err(|_| AppError::Unauthorized)?;
    let recovery_byte =
        normalize_recovery_byte(signature_bytes[64]).ok_or(AppError::Unauthorized)?;
    let recovery_id = RecoveryId::from_byte(recovery_byte).ok_or(AppError::Unauthorized)?;
    let digest = ethereum_message_digest(message);
    let verifying_key = VerifyingKey::recover_from_digest(digest, &signature, recovery_id)
        .map_err(|_| AppError::Unauthorized)?;

    Ok(verifying_key_to_address(&verifying_key))
}

fn normalize_recovery_byte(byte: u8) -> Option<u8> {
    match byte {
        27 | 28 => Some(byte - 27),
        0 | 1 => Some(byte),
        _ => None,
    }
}

fn ethereum_message_digest(message: &str) -> Keccak256 {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut payload = Vec::with_capacity(prefix.len() + message.len());
    payload.extend_from_slice(prefix.as_bytes());
    payload.extend_from_slice(message.as_bytes());
    Keccak256::new_with_prefix(payload)
}

fn verifying_key_to_address(verifying_key: &VerifyingKey) -> String {
    let encoded_point: EncodedPoint = verifying_key.to_encoded_point(false);
    let public_key = encoded_point.as_bytes();
    let hash = keccak256(&public_key[1..]);
    format!("0x{}", encode_hex(&hash[12..]))
}

fn normalize_wallet_address(wallet_address: &str) -> Result<String, AppError> {
    let decoded = decode_hex(wallet_address)
        .map_err(|_| AppError::BadRequest("invalid wallet address".to_owned()))?;

    if decoded.len() != 20 {
        return Err(AppError::BadRequest("invalid wallet address".to_owned()));
    }

    Ok(format!("0x{}", encode_hex(&decoded)))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn decode_hex(input: &str) -> Result<Vec<u8>, ()> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);

    if normalized.len() % 2 != 0 {
        return Err(());
    }

    let mut bytes = Vec::with_capacity(normalized.len() / 2);

    for pair in normalized.as_bytes().chunks_exact(2) {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }

    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}
