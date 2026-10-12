//! Who is calling: the deployment operator, the Station Agent, or a signed-in organization
//! user. Routes that make the platform spend money or the organization key sign resolve the
//! caller here and then check what that caller may do for the custodian involved.

use axum::http::HeaderMap;

use crate::{
    error::ApiError,
    orgs::Role,
    repository::{self, orgs::OrgUser},
    routes::agent::authorize,
    state::AppState,
};

pub(crate) enum Actor {
    Operator,
    Agent,
    User(OrgUser),
}

/// Resolves the bearer token; any unknown token is `Unauthorized`.
pub(crate) async fn actor(state: &AppState, headers: &HeaderMap) -> Result<Actor, ApiError> {
    if let Some(operator) = state.config.operator_token.as_deref()
        && authorize(headers, operator).is_ok()
    {
        return Ok(Actor::Operator);
    }
    if authorize(headers, &state.config.agent_token).is_ok() {
        return Ok(Actor::Agent);
    }
    Ok(Actor::User(
        crate::routes::orgs::authorize_user(state, headers).await?,
    ))
}

/// May this caller drive handling sessions for `custodian`?
///
/// - the operator: only for a custodian the platform knows (an ACTIVE party or an
///   organization wallet), never for an arbitrary key;
/// - an organization user: only for its own organization wallet, with a role that may run
///   sessions;
/// - the Agent: only when `agent_allowed` (it delivers observations and prepares batches).
pub(crate) async fn authorize_custodian(
    state: &AppState,
    headers: &HeaderMap,
    custodian: [u8; 32],
    agent_allowed: bool,
) -> Result<(), ApiError> {
    match actor(state, headers).await? {
        Actor::Operator => {
            if repository::sponsor::is_known_signer(
                &state.db,
                state.config.deployment_id,
                custodian,
            )
            .await?
            {
                Ok(())
            } else {
                Err(ApiError::Conflict(
                    "custodian is not an active party or organization wallet".into(),
                ))
            }
        }
        Actor::Agent if agent_allowed => Ok(()),
        Actor::Agent => Err(ApiError::Forbidden),
        Actor::User(user) => {
            let runs_sessions = matches!(user.role, Role::OrgAdmin | Role::OperadorCurral);
            if user.wallet.to_bytes() == custodian && runs_sessions {
                Ok(())
            } else {
                Err(ApiError::Forbidden)
            }
        }
    }
}
