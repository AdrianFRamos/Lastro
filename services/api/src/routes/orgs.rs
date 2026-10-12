//! Organization accounts without browser wallets (SPEC v2.1, S2).
//!
//! - `POST /api/v2/organizations` (operator token) creates an organization, its custodian
//!   wallet and its first administrator.
//! - `POST /api/v2/auth/otp/request` and `/verify` sign a person in with a one-time code.
//! - `GET /api/v2/me` describes the signed-in user.
//! - `POST /api/v2/organizations/{orgId}/users` (ORG_ADMIN) adds a person with a role.
//! - `POST /api/v2/org/sign` has the organization wallet sign the transaction prepared for a
//!   purpose its role allows, then broadcasts it (sponsored when the platform pays).
//! - `POST /api/v2/organizations/{orgId}/users/{userId}/disable` (ORG_ADMIN) removes a person.
//! - `POST /api/v2/auth/logout` revokes the caller's token.
//! - `POST /api/v2/organizations/{orgId}/export` (ORG_ADMIN, fresh one-time code) hands the key
//!   out of custody, once; the platform stops signing for that organization afterwards.
//!
//! The organization key only ever signs a transaction the API re-derived itself for a named
//! purpose; a caller never supplies transaction bytes.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, StatusCode, header},
};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::str::FromStr;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::SponsorPurpose,
    orgs::{self, Role},
    repository::orgs::{self as repo, NewOrganization, OrgUser},
    routes::{operations::authorize_operator, sponsor},
    sponsor::compile_unsigned,
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub admin_email: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationResponse {
    pub org_id: Uuid,
    pub name: String,
    /// The organization's custodian wallet (base58); register it as the party's wallet.
    pub wallet: String,
    pub admin_user_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddUserRequest {
    pub email: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub org_name: String,
    pub email: String,
    pub role: &'static str,
    pub wallet: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OtpRequest {
    pub email: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OtpVerifyRequest {
    pub email: String,
    pub code: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTokenResponse {
    pub token: String,
    pub expires_at: String,
    pub user: UserResponse,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrgSignRequest {
    pub purpose: SponsorPurpose,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrgSignResponse {
    pub tx_signature: String,
    pub fee_payer: String,
    pub signer: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    /// Must repeat the organization wallet: the export is deliberate, never a stray click.
    pub confirm_wallet: String,
    /// A one-time code requested for this action: a stolen session token alone cannot export.
    pub code: String,
}

/// Carries the secret key: deliberately not `Debug`, so it can never reach a log line.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub wallet: String,
    /// Solana CLI keypair format: 64 bytes, secret then public key.
    pub keypair: Vec<u8>,
}

pub async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let vault = state.vault()?;
    let name = body.name.trim();
    if name.len() < 2 || name.len() > 200 {
        return Err(ApiError::Validation(
            "name must contain 2..=200 characters".into(),
        ));
    }
    let email = normalize_email(&body.admin_email)?;
    let org_id = Uuid::new_v4();
    let admin_user_id = Uuid::new_v4();
    let (wallet, sealed) = vault.generate(org_id)?;
    repo::create_organization(
        &state.db,
        &NewOrganization {
            deployment_id: state.config.deployment_id,
            org_id,
            name,
            wallet,
            sealed_secret: &sealed,
            admin_user_id,
            admin_email: &email,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(OrganizationResponse {
            org_id,
            name: name.into(),
            wallet: wallet.to_string(),
            admin_user_id,
        }),
    ))
}

pub async fn add_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<Uuid>,
    Json(body): Json<AddUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let admin = authorize_admin(&state, &headers, org_id).await?;
    let email = normalize_email(&body.email)?;
    let role = Role::parse(&body.role)?;
    let user_id = Uuid::new_v4();
    repo::add_user(
        &state.db,
        state.config.deployment_id,
        &admin,
        user_id,
        &email,
        role,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            user_id,
            org_id,
            org_name: admin.org_name,
            email,
            role: role.as_str(),
            wallet: admin.wallet.to_string(),
        }),
    ))
}

/// Always 202 for a well-formed e-mail, so the route does not reveal who has an account.
pub async fn request_code(
    State(state): State<AppState>,
    Json(body): Json<OtpRequest>,
) -> Result<StatusCode, ApiError> {
    let vault = state.vault()?;
    if !state.config.otp_log_delivery {
        return Err(ApiError::Unavailable(
            "one-time-code login is not configured".into(),
        ));
    }
    let email = normalize_email(&body.email)?;
    let Some(user) =
        repo::find_active_user_by_email(&state.db, state.config.deployment_id, &email).await?
    else {
        return Ok(StatusCode::ACCEPTED);
    };
    let code = orgs::new_code();
    if repo::issue_code(
        &state.db,
        user.user_id,
        vault.code_hash(user.user_id, &code),
    )
    .await?
    {
        // Development delivery only (LASTRO_OTP_DELIVERY=log); production wires an e-mail
        // or SMS provider here.
        tracing::info!(user = %user.user_id, %code, "one-time login code");
    } else {
        tracing::warn!(user = %user.user_id, "one-time login code rate limited");
    }
    Ok(StatusCode::ACCEPTED)
}

pub async fn verify_code(
    State(state): State<AppState>,
    Json(body): Json<OtpVerifyRequest>,
) -> Result<([(HeaderName, &'static str); 1], Json<SessionTokenResponse>), ApiError> {
    let vault = state.vault()?;
    let email = normalize_email(&body.email)?;
    if !valid_code_format(&body.code) {
        return Err(ApiError::Unauthorized);
    }
    let user = repo::find_active_user_by_email(&state.db, state.config.deployment_id, &email)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if !repo::consume_code(
        &state.db,
        user.user_id,
        vault.code_hash(user.user_id, &body.code),
    )
    .await?
    {
        return Err(ApiError::Unauthorized);
    }
    let token = orgs::new_token();
    let expires_at = repo::insert_token(&state.db, orgs::token_hash(&token), user.user_id).await?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(SessionTokenResponse {
            token,
            expires_at: expires_at
                .format(&Rfc3339)
                .map_err(|_| ApiError::Internal)?,
            user: user_response(&user),
        }),
    ))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, ApiError> {
    Ok(Json(user_response(
        &authorize_user(&state, &headers).await?,
    )))
}

pub async fn sign(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OrgSignRequest>,
) -> Result<Json<OrgSignResponse>, ApiError> {
    let user = authorize_user(&state, &headers).await?;
    if !orgs::role_allows(user.role, &body.purpose) {
        return Err(ApiError::Forbidden);
    }
    let vault = state.vault()?;
    let expected = sponsor::prepare_purpose(&state, &body.purpose, None).await?;
    if expected.required_signer != user.wallet.to_string() {
        return Err(ApiError::Conflict(
            "this transaction is not authorized by the organization wallet".into(),
        ));
    }
    let fee_payer = Pubkey::from_str(&expected.fee_payer).map_err(|_| ApiError::Internal)?;
    let sponsor_key = state.sponsor().map(|sponsor| sponsor.pubkey());
    let sponsored = Some(fee_payer) == sponsor_key && fee_payer != user.wallet;
    if !sponsored && fee_payer != user.wallet {
        return Err(ApiError::Internal);
    }

    let mut transaction = compile_unsigned(&expected, state.rpc.latest_blockhash().await?)?;
    // Only the organization wallet and, when sponsored, the sponsor may be asked to sign.
    let signers = &transaction.account_keys[..usize::from(transaction.num_required_signatures)];
    if signers
        .iter()
        .any(|key| *key != user.wallet && Some(*key) != sponsor_key.filter(|_| sponsored))
    {
        return Err(ApiError::Conflict(
            "the prepared transaction needs a signer outside the organization".into(),
        ));
    }
    let sealed = repo::custodied_secret(&state.db, state.config.deployment_id, user.org_id).await?;
    let key = vault.open(user.org_id, user.wallet, &sealed)?;
    transaction.set_signature(&user.wallet, orgs::sign(&key, &transaction.message))?;
    drop(key);

    let tx_signature = if sponsored {
        sponsor::sponsor_and_send(&state, &body.purpose, &transaction.encode(), &expected)
            .await?
            .tx_signature
    } else {
        let expected_signature = bs58::encode(transaction.signatures[0]).into_string();
        let returned = state.rpc.send_transaction(&transaction.encode()).await?;
        if returned != expected_signature {
            return Err(ApiError::Unavailable(
                "Solana RPC returned a different transaction signature".into(),
            ));
        }
        returned
    };
    repo::record_signature(&state.db, &tx_signature, &user, body.purpose.label()).await?;
    tracing::info!(
        org = %user.org_id,
        user = %user.user_id,
        purpose = body.purpose.label(),
        tx = %tx_signature,
        "organization wallet signed"
    );
    Ok(Json(OrgSignResponse {
        tx_signature,
        fee_payer: fee_payer.to_string(),
        signer: user.wallet.to_string(),
    }))
}

pub async fn export_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<Uuid>,
    Json(body): Json<ExportRequest>,
) -> Result<([(HeaderName, &'static str); 1], Json<ExportResponse>), ApiError> {
    let admin = authorize_admin(&state, &headers, org_id).await?;
    if body.confirm_wallet != admin.wallet.to_string() {
        return Err(ApiError::Validation(
            "confirmWallet must repeat the organization wallet".into(),
        ));
    }
    let vault = state.vault()?;
    if !valid_code_format(&body.code)
        || !repo::consume_code(
            &state.db,
            admin.user_id,
            vault.code_hash(admin.user_id, &body.code),
        )
        .await?
    {
        return Err(ApiError::Unauthorized);
    }
    let sealed = repo::custodied_secret(&state.db, state.config.deployment_id, org_id).await?;
    let key = vault.open(org_id, admin.wallet, &sealed)?;
    repo::mark_exported(&state.db, state.config.deployment_id, &admin).await?;
    tracing::warn!(org = %org_id, user = %admin.user_id, "organization key exported");
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(ExportResponse {
            wallet: admin.wallet.to_string(),
            keypair: key.to_keypair_bytes().to_vec(),
        }),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    authorize_user(&state, &headers).await?;
    let token = bearer(&headers).ok_or(ApiError::Unauthorized)?;
    repo::revoke_token(&state.db, orgs::token_hash(token)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn disable_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let admin = authorize_admin(&state, &headers, org_id).await?;
    if admin.user_id == user_id {
        return Err(ApiError::Conflict(
            "an administrator cannot disable themself".into(),
        ));
    }
    repo::disable_user(&state.db, state.config.deployment_id, &admin, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn valid_code_format(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// The user behind a live bearer token.
pub(crate) async fn authorize_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<OrgUser, ApiError> {
    let token = bearer(headers).ok_or(ApiError::Unauthorized)?;
    repo::user_for_token(
        &state.db,
        state.config.deployment_id,
        orgs::token_hash(token),
    )
    .await?
    .ok_or(ApiError::Unauthorized)
}

async fn authorize_admin(
    state: &AppState,
    headers: &HeaderMap,
    org_id: Uuid,
) -> Result<OrgUser, ApiError> {
    let user = authorize_user(state, headers).await?;
    if user.org_id != org_id || user.role != Role::OrgAdmin {
        return Err(ApiError::Forbidden);
    }
    Ok(user)
}

fn user_response(user: &OrgUser) -> UserResponse {
    UserResponse {
        user_id: user.user_id,
        org_id: user.org_id,
        org_name: user.org_name.clone(),
        email: user.email.clone(),
        role: user.role.as_str(),
        wallet: user.wallet.to_string(),
    }
}

fn normalize_email(value: &str) -> Result<String, ApiError> {
    let email = value.trim().to_ascii_lowercase();
    let valid = (3..=254).contains(&email.len())
        && email.is_ascii()
        && !email
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.contains('@')
        });
    if valid {
        Ok(email)
    } else {
        Err(ApiError::Validation("email is invalid".into()))
    }
}
