//! Organization accounts, one-time login codes and bearer sessions (SPEC v2.1, S2).
//!
//! Only hashes of codes and tokens and the sealed organization secret are stored; nothing in
//! these tables can sign a transaction without the vault's master key.

use serde_json::json;
use solana_pubkey::Pubkey;
use sqlx::{PgPool, Row, postgres::PgRow};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    error::ApiError,
    orgs::{KEY_VERSION, Role, SEALED_SECRET_LEN},
    repository::db_error,
    repository::operations::append_audit_tx,
};

pub const CODE_LIFETIME: Duration = Duration::minutes(10);
pub const CODE_MAX_ATTEMPTS: i16 = 5;
/// At most this many codes per user per [`CODE_RATE_WINDOW`].
pub const CODE_RATE_LIMIT: i64 = 5;
pub const CODE_RATE_WINDOW: Duration = Duration::minutes(15);
pub const TOKEN_LIFETIME: Duration = Duration::hours(12);
/// Wrong codes a user may submit per [`FAILED_VERIFY_WINDOW`] before login is locked.
pub const FAILED_VERIFY_LIMIT: i64 = 10;
/// Short on purpose: a stranger who knows an e-mail can lock that login, but only for an hour.
pub const FAILED_VERIFY_WINDOW: Duration = Duration::hours(1);

#[derive(Clone, Debug)]
pub struct OrgUser {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub role: Role,
    pub org_name: String,
    pub wallet: Pubkey,
}

pub struct NewOrganization<'a> {
    pub deployment_id: [u8; 32],
    pub org_id: Uuid,
    pub name: &'a str,
    pub wallet: Pubkey,
    pub sealed_secret: &'a [u8; SEALED_SECRET_LEN],
    pub admin_user_id: Uuid,
    pub admin_email: &'a str,
}

const USER_SELECT: &str = "SELECT u.user_id,u.org_id,u.email,u.role,o.name,o.wallet FROM v2_org_users u JOIN v2_organizations o ON o.org_id=u.org_id";

pub async fn create_organization(pool: &PgPool, org: &NewOrganization<'_>) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("INSERT INTO v2_organizations(org_id,deployment_id,name,wallet,sealed_secret,key_version) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(org.org_id)
        .bind(org.deployment_id.to_vec())
        .bind(org.name)
        .bind(org.wallet.to_bytes().to_vec())
        .bind(org.sealed_secret.to_vec())
        .bind(KEY_VERSION)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    insert_user_tx(
        &mut tx,
        org.admin_user_id,
        org.org_id,
        org.admin_email,
        Role::OrgAdmin,
    )
    .await?;
    append_audit_tx(
        &mut tx,
        org.deployment_id,
        None,
        Some(org.wallet.to_bytes()),
        "ORGANIZATION_CREATED",
        "organization",
        None,
        json!({ "orgId": org.org_id, "name": org.name, "adminEmail": org.admin_email }),
    )
    .await?;
    tx.commit().await.map_err(db_error)
}

pub async fn add_user(
    pool: &PgPool,
    deployment_id: [u8; 32],
    actor: &OrgUser,
    user_id: Uuid,
    email: &str,
    role: Role,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    insert_user_tx(&mut tx, user_id, actor.org_id, email, role).await?;
    append_audit_tx(
        &mut tx,
        deployment_id,
        None,
        Some(actor.wallet.to_bytes()),
        "ORGANIZATION_USER_ADDED",
        "organization",
        None,
        json!({ "orgId": actor.org_id, "by": actor.user_id, "userId": user_id, "email": email, "role": role.as_str() }),
    )
    .await?;
    tx.commit().await.map_err(db_error)
}

async fn insert_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    org_id: Uuid,
    email: &str,
    role: Role,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO v2_org_users(user_id,org_id,email,role,status) VALUES($1,$2,$3,$4,'ACTIVE')",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(email)
    .bind(role.as_str())
    .execute(&mut **tx)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            ApiError::Conflict("this e-mail already belongs to a user".into())
        }
        other => db_error(other),
    })?;
    Ok(())
}

/// The active user with `email` in this deployment.
pub async fn find_active_user_by_email(
    pool: &PgPool,
    deployment_id: [u8; 32],
    email: &str,
) -> Result<Option<OrgUser>, ApiError> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} WHERE u.email=$1 AND u.status='ACTIVE' AND o.deployment_id=$2"
    )))
    .bind(email)
    .bind(deployment_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .map(|row| decode_user(&row))
    .transpose()
}

/// Stores a new code unless the user already received [`CODE_RATE_LIMIT`] recently.
/// Returns `false` (nothing stored) when rate limited.
pub async fn issue_code(
    pool: &PgPool,
    user_id: Uuid,
    code_hash: [u8; 32],
) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    // Serializes concurrent requests for the same user so the limit cannot be raced.
    sqlx::query("SELECT 1 FROM v2_org_users WHERE user_id=$1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let now = OffsetDateTime::now_utc();
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM v2_login_codes WHERE user_id=$1 AND created_at > $2",
    )
    .bind(user_id)
    .bind(now - CODE_RATE_WINDOW)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if recent >= CODE_RATE_LIMIT {
        return Ok(false);
    }
    // Earlier live codes stay valid: a stranger asking for codes must not lock the real user
    // out. Guessing is bounded by FAILED_VERIFY_LIMIT across all of the user's codes.
    sqlx::query(
        "INSERT INTO v2_login_codes(code_id,user_id,code_hash,expires_at) VALUES($1,$2,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(code_hash.to_vec())
    .bind(now + CODE_LIFETIME)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(true)
}

/// Consumes a live code of the user when `code_hash` matches one; a miss spends an attempt of
/// the newest code (which dies after [`CODE_MAX_ATTEMPTS`]). Once the user has
/// [`FAILED_VERIFY_LIMIT`] misses in [`FAILED_VERIFY_WINDOW`], nothing verifies until the window
/// passes, whatever the code.
pub async fn consume_code(
    pool: &PgPool,
    user_id: Uuid,
    code_hash: [u8; 32],
) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT 1 FROM v2_org_users WHERE user_id=$1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let failures: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(attempts),0)::BIGINT FROM v2_login_codes WHERE user_id=$1 AND created_at > $2",
    )
    .bind(user_id)
    .bind(OffsetDateTime::now_utc() - FAILED_VERIFY_WINDOW)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if failures >= FAILED_VERIFY_LIMIT {
        return Ok(false);
    }
    let rows = sqlx::query(
        "SELECT code_id,code_hash,attempts FROM v2_login_codes WHERE user_id=$1 AND NOT consumed AND expires_at > now() ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(newest) = rows.first() else {
        return Ok(false);
    };
    let mut matched = false;
    for row in &rows {
        let stored: Vec<u8> = row.try_get("code_hash").map_err(db_error)?;
        matched |= bool::from(stored.as_slice().ct_eq(&code_hash));
    }
    if matched {
        // Signing in spends every live code of the user.
        sqlx::query("UPDATE v2_login_codes SET consumed=true WHERE user_id=$1 AND NOT consumed")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    } else {
        let code_id: Uuid = newest.try_get("code_id").map_err(db_error)?;
        let attempts: i16 = newest.try_get::<i16, _>("attempts").map_err(db_error)? + 1;
        sqlx::query("UPDATE v2_login_codes SET attempts=$2, consumed=$3 WHERE code_id=$1")
            .bind(code_id)
            .bind(attempts)
            .bind(attempts >= CODE_MAX_ATTEMPTS)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(matched)
}

pub async fn insert_token(
    pool: &PgPool,
    token_hash: [u8; 32],
    user_id: Uuid,
) -> Result<OffsetDateTime, ApiError> {
    let expires_at = OffsetDateTime::now_utc() + TOKEN_LIFETIME;
    sqlx::query("INSERT INTO v2_user_tokens(token_hash,user_id,expires_at) VALUES($1,$2,$3)")
        .bind(token_hash.to_vec())
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(expires_at)
}

/// The active user a live, unrevoked token belongs to.
pub async fn user_for_token(
    pool: &PgPool,
    deployment_id: [u8; 32],
    token_hash: [u8; 32],
) -> Result<Option<OrgUser>, ApiError> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} JOIN v2_user_tokens t ON t.user_id=u.user_id WHERE t.token_hash=$1 AND NOT t.revoked AND t.expires_at > now() AND u.status='ACTIVE' AND o.deployment_id=$2"
    )))
    .bind(token_hash.to_vec())
    .bind(deployment_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .map(|row| decode_user(&row))
    .transpose()
}

/// The sealed key of an organization whose key is still in managed custody. Once exported,
/// the platform no longer signs for it: the key would otherwise live in two places.
pub async fn custodied_secret(
    pool: &PgPool,
    deployment_id: [u8; 32],
    org_id: Uuid,
) -> Result<[u8; SEALED_SECRET_LEN], ApiError> {
    let row = sqlx::query(
        "SELECT sealed_secret, exported_at IS NOT NULL AS exported FROM v2_organizations WHERE org_id=$1 AND deployment_id=$2",
    )
    .bind(org_id)
    .bind(deployment_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::NotFound("organization not found".into()))?;
    if row.try_get::<bool, _>("exported").map_err(db_error)? {
        return Err(ApiError::Conflict(
            "the organization key was exported; sign with that wallet".into(),
        ));
    }
    let sealed: Vec<u8> = row.try_get("sealed_secret").map_err(db_error)?;
    sealed.try_into().map_err(|_| ApiError::Internal)
}

pub async fn record_signature(
    pool: &PgPool,
    tx_signature: &str,
    user: &OrgUser,
    purpose: &str,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO v2_org_signatures(tx_signature,org_id,user_id,purpose) VALUES($1,$2,$3,$4) ON CONFLICT (tx_signature) DO NOTHING")
        .bind(tx_signature)
        .bind(user.org_id)
        .bind(user.user_id)
        .bind(purpose)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

/// Records that the administrator took the key out of custody. Exactly once: a second export
/// fails, and the audit entry is written in the same transaction.
pub async fn mark_exported(
    pool: &PgPool,
    deployment_id: [u8; 32],
    actor: &OrgUser,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let updated = sqlx::query(
        "UPDATE v2_organizations SET exported_at=now() WHERE org_id=$1 AND exported_at IS NULL",
    )
    .bind(actor.org_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Conflict(
            "the organization key was already exported".into(),
        ));
    }
    append_audit_tx(
        &mut tx,
        deployment_id,
        None,
        Some(actor.wallet.to_bytes()),
        "ORGANIZATION_KEY_EXPORTED",
        "organization",
        None,
        json!({ "orgId": actor.org_id, "by": actor.user_id }),
    )
    .await?;
    tx.commit().await.map_err(db_error)
}

fn decode_user(row: &PgRow) -> Result<OrgUser, ApiError> {
    let wallet: Vec<u8> = row.try_get("wallet").map_err(db_error)?;
    let wallet: [u8; 32] = wallet.try_into().map_err(|_| ApiError::Internal)?;
    let role: String = row.try_get("role").map_err(db_error)?;
    Ok(OrgUser {
        user_id: row.try_get("user_id").map_err(db_error)?,
        org_id: row.try_get("org_id").map_err(db_error)?,
        email: row.try_get("email").map_err(db_error)?,
        role: Role::parse(&role).map_err(|_| ApiError::Internal)?,
        org_name: row.try_get("name").map_err(db_error)?,
        wallet: Pubkey::new_from_array(wallet),
    })
}

pub async fn revoke_token(pool: &PgPool, token_hash: [u8; 32]) -> Result<(), ApiError> {
    sqlx::query("UPDATE v2_user_tokens SET revoked=true WHERE token_hash=$1")
        .bind(token_hash.to_vec())
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

/// Disables a user of the administrator's organization and revokes every token of that user.
pub async fn disable_user(
    pool: &PgPool,
    deployment_id: [u8; 32],
    admin: &OrgUser,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let updated = sqlx::query(
        "UPDATE v2_org_users SET status='DISABLED' WHERE user_id=$1 AND org_id=$2 AND status='ACTIVE'",
    )
    .bind(user_id)
    .bind(admin.org_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::NotFound(
            "active user not found in this organization".into(),
        ));
    }
    sqlx::query("UPDATE v2_user_tokens SET revoked=true WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    append_audit_tx(
        &mut tx,
        deployment_id,
        None,
        Some(admin.wallet.to_bytes()),
        "ORGANIZATION_USER_DISABLED",
        "organization",
        None,
        json!({ "orgId": admin.org_id, "by": admin.user_id, "userId": user_id }),
    )
    .await?;
    tx.commit().await.map_err(db_error)
}
