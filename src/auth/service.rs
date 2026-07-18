use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use chrono::{DateTime, Utc};
use k256::{
    EncodedPoint,
    ecdsa::{RecoveryId, Signature, VerifyingKey},
};
use rand_core::OsRng;
use sha3::{Digest, Keccak256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    auth::dto::{
        AuthSessionResponse, AuthUserResponse, CreateChallengeResponse, ForgotPasswordRequest,
        LoginRequest, LogoutResponse, ResetPasswordRequest, SettingsUpdateResponse, SignupRequest,
        UpdateEmailRequest, UpdatePasswordRequest, UpdateUsernameRequest, VenueConnectionResponse,
        VerifyChallengeResponse, WalletChallengeResponse, account_warning_for_connection,
    },
    db::Db,
    entities::{auth_method, user, user_session, venue_connection, wallet_signature_challenge},
    error::AppError,
    libs::{
        credentials::{encrypt_json, parse_encryption_key},
        resend_client::send_email,
    },
    providers::{
        polymarket::{
            connection::{ACTIVE_STATUS, reconcile_polymarket_after_wallet_replacement},
            credentials::{ConnectPolymarketRequest, PolymarketSignatureType},
        },
        types::{Chain, ChainId, DEFAULT_PROVIDER, ProviderId},
    },
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, SqlErr,
    TransactionTrait, sea_query::OnConflict,
};
use serde_json::{Value, json};

const CHALLENGE_TTL_SECONDS: u64 = 300;
const WALLET_ASSOCIATION_PURPOSE: &str = "associate_wallet";

const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24 * 30;
const EMAIL_VERIFICATION_TTL_SECONDS: u64 = 60 * 60 * 24;
const PASSWORD_RESET_TTL_SECONDS: u64 = 60 * 60;
const MIN_PASSWORD_LENGTH: usize = 8;
const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 20;
const RESERVED_USERNAMES: [&str; 9] = [
    "admin",
    "administrator",
    "api",
    "support",
    "uptions",
    "root",
    "system",
    "settings",
    "profile",
];

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

    pub async fn create_wallet_challenge(
        &self,
        access_token: &str,
        wallet_address: &str,
        provider: ProviderId,
        chain: Chain,
        chain_id: ChainId,
    ) -> Result<WalletChallengeResponse, AppError> {
        validate_provider_chain(provider, chain, chain_id)?;
        let user_id = self.session(access_token).await?.user_id;
        self.ensure_selected_provider(&user_id, provider).await?;
        let wallet_address = normalize_wallet_address(wallet_address)?;
        self.ensure_wallet_available(&user_id, &wallet_address)
            .await?;

        let nonce = Uuid::new_v4().to_string();
        let expires_at = unix_timestamp() + CHALLENGE_TTL_SECONDS;
        let message = wallet_association_message(
            &user_id,
            provider,
            chain,
            chain_id,
            &wallet_address,
            &nonce,
            expires_at,
        );

        wallet_signature_challenge::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id),
            purpose: Set(WALLET_ASSOCIATION_PURPOSE.to_owned()),
            provider: Set(provider.storage_value().to_owned()),
            chain: Set(chain.storage_value().to_owned()),
            chain_id: Set(chain_id.value() as i64),
            wallet_address: Set(wallet_address.clone()),
            nonce_hash: Set(hash_access_token(&nonce)),
            expires_at: Set(DateTime::<Utc>::from_timestamp(expires_at as i64, 0)
                .expect("wallet challenge expiry must be a valid timestamp")
                .into()),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        Ok(WalletChallengeResponse {
            purpose: WALLET_ASSOCIATION_PURPOSE.to_owned(),
            provider,
            chain,
            chain_id,
            wallet_address,
            nonce,
            message,
            expires_at,
        })
    }

    pub async fn signup(&self, payload: SignupRequest) -> Result<AuthUserResponse, AppError> {
        let email = normalize_email(&payload.email)?;
        let username = normalize_username(&payload.username)?;
        validate_password(&payload.password)?;

        let existing_email = user::Entity::find()
            .filter(user::Column::Email.eq(Some(email.clone())))
            .one(&self.db)
            .await?;

        if existing_email.is_some() {
            return Err(AppError::Conflict("email is already registered".to_owned()));
        }

        let existing_username = user::Entity::find()
            .filter(user::Column::Username.eq(Some(username.clone())))
            .one(&self.db)
            .await?;

        if existing_username.is_some() {
            return Err(AppError::Conflict(
                "username is already registered".to_owned(),
            ));
        }

        let password_hash = hash_password(&payload.password)?;
        let verification_token = generate_auth_token();
        let verification_expires_at =
            timestamp_after(Duration::from_secs(EMAIL_VERIFICATION_TTL_SECONDS));
        let user_id = Uuid::new_v4().to_string();
        let user = user::ActiveModel {
            id: Set(user_id.clone()),
            email: Set(Some(email.clone())),
            username: Set(Some(username)),
            password_hash: Set(Some(password_hash)),
            preferred_trading_provider: Set(DEFAULT_PROVIDER.storage_value().to_owned()),
            email_verification_token_hash: Set(Some(hash_access_token(&verification_token))),
            email_verification_expires_at: Set(Some(verification_expires_at.into())),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                AppError::Conflict("email or username is already registered".to_owned())
            }
            _ => AppError::DatabaseError(error.to_string()),
        })?;

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

        let txn = self.db.begin().await?;
        let mut active = user.into_active_model();
        active.password_hash = Set(Some(hash_password(&payload.password)?));
        active.password_reset_token_hash = Set(None);
        active.password_reset_expires_at = Set(None);
        active.email_verified_at = Set(Some(Utc::now().into()));
        let user = active.update(&txn).await?;
        user_session::Entity::delete_many()
            .filter(user_session::Column::UserId.eq(&user.id))
            .exec(&txn)
            .await?;
        txn.commit().await?;

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

    pub async fn associate_wallet(
        &self,
        access_token: &str,
        wallet_address: &str,
        provider: ProviderId,
        chain: Chain,
        chain_id: ChainId,
        nonce: &str,
        signature: &str,
    ) -> Result<String, AppError> {
        validate_provider_chain(provider, chain, chain_id)?;
        let user_id = self.session(access_token).await?.user_id;
        self.ensure_selected_provider(&user_id, provider).await?;
        let wallet_address = normalize_wallet_address(wallet_address)?;
        let nonce = normalize_token(nonce)?;
        let challenge = wallet_signature_challenge::Entity::find()
            .filter(wallet_signature_challenge::Column::NonceHash.eq(hash_access_token(nonce)))
            .one(&self.db)
            .await?
            .ok_or_else(invalid_wallet_challenge)?;
        let expires_at = challenge.expires_at.with_timezone(&Utc);

        if challenge.user_id != user_id
            || challenge.purpose != WALLET_ASSOCIATION_PURPOSE
            || challenge.provider != provider.storage_value()
            || challenge.chain != chain.storage_value()
            || challenge.chain_id != chain_id.value() as i64
            || challenge.wallet_address != wallet_address
            || challenge.used_at.is_some()
            || expires_at <= Utc::now()
        {
            return Err(invalid_wallet_challenge());
        }

        let expires_at_unix = expires_at.timestamp() as u64;
        let message = wallet_association_message(
            &user_id,
            provider,
            chain,
            chain_id,
            &wallet_address,
            nonce,
            expires_at_unix,
        );
        let recovered_address = recover_wallet_address(&message, signature)?;
        if recovered_address != wallet_address {
            return Err(AppError::Unauthorized);
        }

        let txn = self.db.begin().await?;
        let now = Utc::now();
        let consumed = wallet_signature_challenge::Entity::update_many()
            .set(wallet_signature_challenge::ActiveModel {
                used_at: Set(Some(now.into())),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .filter(wallet_signature_challenge::Column::Id.eq(challenge.id))
            .filter(wallet_signature_challenge::Column::UsedAt.is_null())
            .filter(wallet_signature_challenge::Column::ExpiresAt.gt(now))
            .exec(&txn)
            .await?;

        if consumed.rows_affected != 1 {
            return Err(invalid_wallet_challenge());
        }

        if user::Entity::find()
            .filter(user::Column::PrimaryWalletAddress.eq(Some(wallet_address.clone())))
            .filter(user::Column::Id.ne(&user_id))
            .one(&txn)
            .await?
            .is_some()
            || auth_method::Entity::find()
                .filter(auth_method::Column::MethodType.eq("wallet"))
                .filter(auth_method::Column::ExternalId.eq(&wallet_address))
                .filter(auth_method::Column::UserId.ne(&user_id))
                .one(&txn)
                .await?
                .is_some()
        {
            return Err(AppError::Conflict(
                "wallet is already associated with another account".to_owned(),
            ));
        }

        let model = user::Entity::find_by_id(&user_id)
            .one(&txn)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let mut active = model.into_active_model();
        active.primary_wallet_address = Set(Some(wallet_address.clone()));
        active.updated_at = Set(now.into());
        active
            .update(&txn)
            .await
            .map_err(wallet_association_error)?;

        reconcile_polymarket_after_wallet_replacement(&txn, &user_id, &wallet_address, now).await?;

        if let Some(model) = auth_method::Entity::find()
            .filter(auth_method::Column::UserId.eq(&user_id))
            .filter(auth_method::Column::MethodType.eq("wallet"))
            .one(&txn)
            .await?
        {
            let mut active = model.into_active_model();
            active.external_id = Set(wallet_address.clone());
            active.updated_at = Set(now.into());
            active
                .update(&txn)
                .await
                .map_err(wallet_association_error)?;
        } else {
            auth_method::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                user_id: Set(user_id),
                method_type: Set("wallet".to_owned()),
                external_id: Set(wallet_address.clone()),
                meta: Set(json!({
                    "provider": provider,
                    "chain": chain,
                    "chain_id": chain_id,
                    "verified_by": WALLET_ASSOCIATION_PURPOSE
                })),
                ..Default::default()
            }
            .insert(&txn)
            .await
            .map_err(wallet_association_error)?;
        }

        txn.commit().await?;
        Ok(wallet_address)
    }

    pub async fn logout(&self, access_token: &str) -> Result<LogoutResponse, AppError> {
        self.session(access_token).await?;
        let result = user_session::Entity::delete_many()
            .filter(user_session::Column::TokenHash.eq(hash_access_token(access_token)))
            .exec(&self.db)
            .await?;

        Ok(LogoutResponse {
            revoked_sessions: result.rows_affected,
        })
    }

    pub async fn logout_all(&self, access_token: &str) -> Result<LogoutResponse, AppError> {
        let user_id = self.session(access_token).await?.user_id;
        let revoked_sessions = self.revoke_user_sessions(&user_id).await?;

        Ok(LogoutResponse { revoked_sessions })
    }

    pub async fn current_user_id(&self, access_token: &str) -> Result<String, AppError> {
        Ok(self.session(access_token).await?.user_id)
    }

    pub async fn current_user(&self, access_token: &str) -> Result<AuthUserResponse, AppError> {
        let session = self.session(access_token).await?;
        let user = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        Ok(self.auth_user_response(&user).await?)
    }

    pub async fn update_email(
        &self,
        access_token: &str,
        payload: UpdateEmailRequest,
    ) -> Result<AuthUserResponse, AppError> {
        let session = self.session(access_token).await?;
        let email = normalize_email(&payload.email)?;
        let model = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if let Some(password_hash) = model.password_hash.as_deref() {
            let current_password = payload
                .current_password
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("current password is required".to_owned()))?;

            if !verify_password(current_password, password_hash)? {
                return Err(AppError::Unauthorized);
            }
        }

        if model.email.as_deref() == Some(email.as_str()) {
            return self.auth_user_response(&model).await;
        }

        let existing = user::Entity::find()
            .filter(user::Column::Email.eq(Some(email.clone())))
            .one(&self.db)
            .await?;

        if existing.is_some_and(|existing| existing.id != model.id) {
            return Err(AppError::Conflict("email is already registered".to_owned()));
        }

        let verification_token = generate_auth_token();
        let verification_expires_at =
            timestamp_after(Duration::from_secs(EMAIL_VERIFICATION_TTL_SECONDS));
        let user_id = model.id.clone();
        let mut active = model.into_active_model();
        active.email = Set(Some(email.clone()));
        active.email_verified_at = Set(None);
        active.email_verification_token_hash = Set(Some(hash_access_token(&verification_token)));
        active.email_verification_expires_at = Set(Some(verification_expires_at.into()));
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&self.db).await?;
        self.sync_email_auth_method(&user_id, &email).await?;
        self.send_verification_email(&email, &verification_token)
            .await;

        self.auth_user_response(&model).await
    }

    pub async fn update_password(
        &self,
        access_token: &str,
        payload: UpdatePasswordRequest,
    ) -> Result<SettingsUpdateResponse, AppError> {
        let session = self.session(access_token).await?;
        let model = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let password_hash = model.password_hash.as_deref().ok_or_else(|| {
            AppError::BadRequest("password is not configured for this account".to_owned())
        })?;

        if !verify_password(&payload.current_password, password_hash)? {
            return Err(AppError::Unauthorized);
        }

        validate_password(&payload.new_password)?;
        let txn = self.db.begin().await?;
        let mut active = model.into_active_model();
        active.password_hash = Set(Some(hash_password(&payload.new_password)?));
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        user_session::Entity::delete_many()
            .filter(user_session::Column::UserId.eq(&session.user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;

        Ok(SettingsUpdateResponse {
            message: "Password updated successfully. Sign in again on your devices.".to_owned(),
        })
    }

    pub async fn update_username(
        &self,
        access_token: &str,
        payload: UpdateUsernameRequest,
    ) -> Result<AuthUserResponse, AppError> {
        let session = self.session(access_token).await?;
        let model = user::Entity::find_by_id(&session.user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let Some(username) =
            requested_username_change(model.username.as_deref(), &payload.username)?
        else {
            return self.auth_user_response(&model).await;
        };

        let existing = user::Entity::find()
            .filter(user::Column::Username.eq(Some(username.clone())))
            .one(&self.db)
            .await?;

        if existing.is_some_and(|existing| existing.id != model.id) {
            return Err(AppError::Conflict(
                "username is already registered".to_owned(),
            ));
        }

        let mut active = model.into_active_model();
        active.username = Set(Some(username));
        active.updated_at = Set(Utc::now().into());
        let model = active
            .update(&self.db)
            .await
            .map_err(username_update_error)?;

        self.auth_user_response(&model).await
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
        let provider = ProviderId::Polymarket;
        ensure_selected_provider(&user, provider)?;

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
            .transpose()?
            .unwrap_or_else(|| account_identifier.clone());
        if funder != account_identifier {
            return Err(AppError::BadRequest(
                "EOA Polymarket funder must match account_identifier".to_owned(),
            ));
        }
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
            .filter(venue_connection::Column::Provider.eq(provider.storage_value()))
            .one(&self.db)
            .await?;

        let connection = match existing {
            Some(model) => {
                let mut active = model.into_active_model();
                active.provider = Set(provider.storage_value().to_owned());
                active.venue = Set(provider.route_value().to_owned());
                active.account_identifier = Set(account_identifier);
                active.config = Set(config);
                restore_polymarket_connection(&mut active);
                active.limits = Set(limits);
                active.auth_type = Set("api_key".to_owned());
                active.permissions = Set(permissions);
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await?
            }
            None => {
                venue_connection::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    user_id: Set(session.user_id),
                    provider: Set(provider.storage_value().to_owned()),
                    venue: Set(provider.route_value().to_owned()),
                    account_identifier: Set(account_identifier),
                    auth_type: Set("api_key".to_owned()),
                    config: Set(config),
                    enabled: Set(true),
                    limits: Set(limits),
                    permissions: Set(permissions),
                    status: Set(ACTIVE_STATUS.to_owned()),
                    ..Default::default()
                }
                .insert(&self.db)
                .await?
            }
        };

        Ok(venue_connection_response(connection))
    }

    async fn ensure_selected_provider(
        &self,
        user_id: &str,
        provider: ProviderId,
    ) -> Result<(), AppError> {
        let user = user::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        ensure_selected_provider(&user, provider)
    }

    async fn ensure_wallet_available(
        &self,
        user_id: &str,
        wallet_address: &str,
    ) -> Result<(), AppError> {
        let assigned_user = user::Entity::find()
            .filter(user::Column::PrimaryWalletAddress.eq(Some(wallet_address.to_owned())))
            .one(&self.db)
            .await?;
        let assigned_method = auth_method::Entity::find()
            .filter(auth_method::Column::MethodType.eq("wallet"))
            .filter(auth_method::Column::ExternalId.eq(wallet_address))
            .one(&self.db)
            .await?;

        if assigned_user.is_some_and(|model| model.id != user_id)
            || assigned_method.is_some_and(|model| model.user_id != user_id)
        {
            return Err(AppError::Conflict(
                "wallet is already associated with another account".to_owned(),
            ));
        }

        Ok(())
    }

    async fn revoke_user_sessions(&self, user_id: &str) -> Result<u64, AppError> {
        let result = user_session::Entity::delete_many()
            .filter(user_session::Column::UserId.eq(user_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
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
            preferred_trading_provider: Set(DEFAULT_PROVIDER.storage_value().to_owned()),
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

    async fn sync_email_auth_method(&self, user_id: &str, email: &str) -> Result<(), AppError> {
        if let Some(model) = auth_method::Entity::find()
            .filter(auth_method::Column::UserId.eq(user_id))
            .filter(auth_method::Column::MethodType.eq("email"))
            .one(&self.db)
            .await?
        {
            let mut active = model.into_active_model();
            active.external_id = Set(email.to_owned());
            active.updated_at = Set(Utc::now().into());
            active.update(&self.db).await?;
            return Ok(());
        }

        self.ensure_email_auth_method(user_id, email).await
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
        let connections = venue_connection::Entity::find()
            .filter(venue_connection::Column::UserId.eq(&user.id))
            .all(&self.db)
            .await?;
        let account_warnings = connections
            .iter()
            .filter_map(|connection| {
                account_warning_for_connection(
                    &connection.provider,
                    connection.enabled,
                    &connection.status,
                )
            })
            .collect();
        let venue_connections = connections
            .into_iter()
            .map(venue_connection_response)
            .collect();

        Ok(AuthUserResponse {
            id: user.id.clone(),
            primary_wallet_address: user.primary_wallet_address.clone(),
            wallet_address: user.primary_wallet_address.clone(),
            email: user.email.clone(),
            username: user.username.clone(),
            email_verified: user.email.is_none() || user.email_verified_at.is_some(),
            password_configured: user.password_hash.is_some(),
            preferred_trading_provider: ProviderId::from_storage(&user.preferred_trading_provider)
                .expect("preferred provider is backfilled before serving requests"),
            venue_connections,
            account_warnings,
        })
    }
}

struct CreatedSession {
    access_token: String,
    expires_at: i64,
}

fn ensure_selected_provider(user: &user::Model, provider: ProviderId) -> Result<(), AppError> {
    let selected = ProviderId::from_storage(&user.preferred_trading_provider)
        .ok_or_else(|| AppError::BadRequest("selected provider is invalid".to_owned()))?;
    if selected != provider {
        return Err(AppError::ProviderValidation {
            code: "SELECTED_PROVIDER_MISMATCH",
            message: "provider action must match the selected provider".to_owned(),
        });
    }
    Ok(())
}

fn wallet_association_message(
    user_id: &str,
    provider: ProviderId,
    chain: Chain,
    chain_id: ChainId,
    wallet_address: &str,
    nonce: &str,
    expires_at: u64,
) -> String {
    format!(
        "Uptions Wallet Association\nPurpose: {WALLET_ASSOCIATION_PURPOSE}\nUser ID: {user_id}\nProvider: {}\nChain: {}\nChain ID: {}\nWallet: {wallet_address}\nNonce: {nonce}\nExpires At: {expires_at}",
        provider.api_value(),
        chain.api_value(),
        chain_id.value()
    )
}

fn validate_provider_chain(
    provider: ProviderId,
    chain: Chain,
    chain_id: ChainId,
) -> Result<(), AppError> {
    let expected_chain = match provider {
        ProviderId::Polymarket => Chain::Polygon,
    };
    if chain != expected_chain || chain_id != expected_chain.id() {
        return Err(AppError::ProviderValidation {
            code: "PROVIDER_CHAIN_MISMATCH",
            message: "wallet chain is not supported by selected provider".to_owned(),
        });
    }
    Ok(())
}

fn invalid_wallet_challenge() -> AppError {
    AppError::BadRequest("wallet challenge is invalid, expired, or already used".to_owned())
}

fn wallet_association_error(error: sea_orm::DbErr) -> AppError {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => {
            AppError::Conflict("wallet is already associated with another account".to_owned())
        }
        _ => AppError::DatabaseError(error.to_string()),
    }
}

fn username_update_error(error: sea_orm::DbErr) -> AppError {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => {
            AppError::Conflict("username is already registered".to_owned())
        }
        _ => AppError::DatabaseError(error.to_string()),
    }
}

fn restore_polymarket_connection(active: &mut venue_connection::ActiveModel) {
    active.enabled = Set(true);
    active.status = Set(ACTIVE_STATUS.to_owned());
}

fn venue_connection_response(model: venue_connection::Model) -> VenueConnectionResponse {
    let provider = ProviderId::from_storage(&model.provider)
        .expect("persisted venue connection provider must be canonical");
    VenueConnectionResponse {
        id: model.id,
        provider,
        venue: provider.route_value().to_owned(),
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

fn polymarket_signature_type(
    signature_type: Option<PolymarketSignatureType>,
) -> Result<i32, AppError> {
    let signature_type = signature_type.unwrap_or(PolymarketSignatureType::Eoa);
    if signature_type != PolymarketSignatureType::Eoa {
        return Err(AppError::BadRequest(
            "Polymarket private beta supports signature_type 0 (EOA) only".to_owned(),
        ));
    }
    Ok(signature_type.value())
}

pub fn normalize_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();

    if email.is_empty() || !email.contains('@') || email.len() > 255 {
        return Err(AppError::BadRequest("valid email is required".to_owned()));
    }

    Ok(email)
}

pub fn normalize_username(username: &str) -> Result<String, AppError> {
    let username = username.trim().to_ascii_lowercase();

    if !(MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH).contains(&username.len())
        || !username.is_ascii()
    {
        return Err(AppError::BadRequest(format!(
            "username must be {MIN_USERNAME_LENGTH}-{MAX_USERNAME_LENGTH} ASCII characters"
        )));
    }

    if !username.as_bytes()[0].is_ascii_lowercase() {
        return Err(AppError::BadRequest(
            "username must start with a letter".to_owned(),
        ));
    }

    if !username
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(AppError::BadRequest(
            "username may only contain lowercase letters, digits, and underscores".to_owned(),
        ));
    }

    if username.ends_with('_') {
        return Err(AppError::BadRequest(
            "username must not end with an underscore".to_owned(),
        ));
    }

    if username.contains("__") {
        return Err(AppError::BadRequest(
            "username must not contain consecutive underscores".to_owned(),
        ));
    }

    if RESERVED_USERNAMES.contains(&username.as_str()) {
        return Err(AppError::BadRequest("username is reserved".to_owned()));
    }

    Ok(username)
}

fn requested_username_change(
    current_username: Option<&str>,
    requested_username: &str,
) -> Result<Option<String>, AppError> {
    let username = normalize_username(requested_username)?;

    if current_username == Some(username.as_str()) {
        Ok(None)
    } else {
        Ok(Some(username))
    }
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters"
        )));
    }

    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
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

#[cfg(test)]
mod tests {
    use k256::ecdsa::SigningKey;

    use crate::{
        providers::polymarket::credentials::PolymarketSignatureType,
        providers::types::{Chain, ChainId, ProviderId},
    };

    use super::{
        RESERVED_USERNAMES, encode_hex, ethereum_message_digest, normalize_username,
        polymarket_signature_type, recover_wallet_address, requested_username_change,
        restore_polymarket_connection, verifying_key_to_address, wallet_association_message,
    };

    #[test]
    fn reconnect_restores_polymarket_connection_to_active() {
        let mut active = crate::entities::venue_connection::ActiveModel {
            enabled: sea_orm::Set(false),
            status: sea_orm::Set("action_required_wallet_mismatch".to_owned()),
            ..Default::default()
        };

        restore_polymarket_connection(&mut active);

        assert_eq!(active.enabled, sea_orm::Set(true));
        assert_eq!(active.status, sea_orm::Set("active".to_owned()));
    }

    #[test]
    fn private_beta_accepts_only_eoa_signature_type() {
        assert_eq!(polymarket_signature_type(None).unwrap(), 0);
        assert_eq!(
            polymarket_signature_type(Some(PolymarketSignatureType::Eoa)).unwrap(),
            0
        );
        assert!(polymarket_signature_type(Some(PolymarketSignatureType::PolyProxy)).is_err());
        assert!(polymarket_signature_type(Some(PolymarketSignatureType::GnosisSafe)).is_err());
    }

    #[test]
    fn wallet_association_message_binds_identity_context() {
        let message = wallet_association_message(
            "user-123",
            ProviderId::Polymarket,
            Chain::Polygon,
            ChainId::POLYGON,
            "0x1111111111111111111111111111111111111111",
            "nonce-456",
            1_760_000_000,
        );

        assert_eq!(
            message,
            "Uptions Wallet Association\nPurpose: associate_wallet\nUser ID: user-123\nProvider: POLYMARKET\nChain: POLYGON\nChain ID: 137\nWallet: 0x1111111111111111111111111111111111111111\nNonce: nonce-456\nExpires At: 1760000000"
        );
    }

    #[test]
    fn wallet_signature_cannot_be_reused_for_another_nonce() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32].into()).unwrap();
        let wallet_address = verifying_key_to_address(signing_key.verifying_key());
        let message = wallet_association_message(
            "user-123",
            ProviderId::Polymarket,
            Chain::Polygon,
            ChainId::POLYGON,
            &wallet_address,
            "nonce-456",
            1_760_000_000,
        );
        let (signature, recovery_id) = signing_key
            .sign_digest_recoverable(ethereum_message_digest(&message))
            .unwrap();
        let mut signature_bytes = signature.to_bytes().to_vec();
        signature_bytes.push(recovery_id.to_byte() + 27);
        let signature = format!("0x{}", encode_hex(&signature_bytes));

        assert_eq!(
            recover_wallet_address(&message, &signature).unwrap(),
            wallet_address
        );

        let altered_message = wallet_association_message(
            "user-123",
            ProviderId::Polymarket,
            Chain::Polygon,
            ChainId::POLYGON,
            &wallet_address,
            "nonce-789",
            1_760_000_000,
        );
        assert_ne!(
            recover_wallet_address(&altered_message, &signature).unwrap(),
            wallet_address
        );
    }

    #[test]
    fn username_change_supports_creation_and_replacement() {
        assert_eq!(
            requested_username_change(None, "  Alice_123  ").unwrap(),
            Some("alice_123".to_owned())
        );
        assert_eq!(
            requested_username_change(Some("alice_123"), "Bob_456").unwrap(),
            Some("bob_456".to_owned())
        );
    }

    #[test]
    fn same_normalized_username_is_idempotent() {
        assert_eq!(
            requested_username_change(Some("alice_123"), "  ALICE_123 ").unwrap(),
            None
        );
    }

    #[test]
    fn username_is_trimmed_and_lowercased() {
        assert_eq!(normalize_username("  Alice_123  ").unwrap(), "alice_123");
    }

    #[test]
    fn username_accepts_valid_boundaries_and_characters() {
        assert_eq!(normalize_username("abc").unwrap(), "abc");
        assert_eq!(normalize_username("a1_b2").unwrap(), "a1_b2");
        assert_eq!(
            normalize_username("abcdefghijklmnopqrst").unwrap(),
            "abcdefghijklmnopqrst"
        );
    }

    #[test]
    fn username_rejects_invalid_lengths_and_non_ascii() {
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("abcdefghijklmnopqrstu").is_err());
        assert!(normalize_username("josé").is_err());
    }

    #[test]
    fn username_rejects_invalid_start_and_characters() {
        assert!(normalize_username("1alice").is_err());
        assert!(normalize_username("_alice").is_err());
        assert!(normalize_username("alice-smith").is_err());
        assert!(normalize_username("alice smith").is_err());
    }

    #[test]
    fn username_rejects_invalid_underscore_placement() {
        assert!(normalize_username("alice_").is_err());
        assert!(normalize_username("alice__smith").is_err());
    }

    #[test]
    fn username_rejects_reserved_names_after_normalization() {
        for username in RESERVED_USERNAMES {
            assert!(normalize_username(&format!(" {username} ")).is_err());
        }
    }
}
