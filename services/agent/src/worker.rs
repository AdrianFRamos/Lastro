use std::time::Duration;

use bytes::Bytes;
use lastro_protocol::{
    Action, StationEvent,
    crypto::{derive_station_id, verify_station_signature},
    rfid::hash_canonical_rfid,
};
use tokio::time::{sleep, timeout};

const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);

use crate::{
    api_client::{ApiClient, EvidenceStatus},
    command::StationCommand,
    error::AgentError,
    serial::{
        StationTransport,
        frame::{Frame, MessageType},
        payload::{
            AckPayload, EventReadyPayload, decode_error, decode_event_ready, encode_ack,
            encode_command,
        },
    },
    spool::{
        Spool,
        model::{OutboxRow, OutboxState},
    },
};

/// Run one Station capture at a time. Evidence reaches durable LOCAL state before ACK or HTTP.
pub async fn run<T: StationTransport>(
    mut transport: T,
    spool: &Spool,
    api: &ApiClient,
    expected_station_pubkey33: [u8; 33],
    poll_interval: Duration,
    station_response_timeout: Duration,
) -> Result<(), AgentError> {
    replay_durable_acks(&mut transport, spool, &expected_station_pubkey33).await?;
    loop {
        match retry_pending_once(spool, api, poll_interval).await {
            Ok(true) => {
                sleep(poll_interval).await;
                continue;
            }
            Ok(false) => {}
            Err(AgentError::Api(message)) => {
                tracing::warn!(error = %message, "Agent API retry pass failed transiently; retrying");
                sleep(poll_interval).await;
                continue;
            }
            Err(error) => return Err(error),
        }

        let command = match api.poll_command().await {
            Ok(command) => command,
            Err(AgentError::Api(message)) => {
                tracing::warn!(error = %message, "Agent command poll failed transiently; retrying");
                sleep(poll_interval).await;
                continue;
            }
            Err(error) => return Err(error),
        };
        let Some(command) = command else {
            sleep(poll_interval).await;
            continue;
        };
        match process_capture(
            &mut transport,
            spool,
            api,
            &expected_station_pubkey33,
            station_response_timeout,
            &command,
        )
        .await
        {
            Ok(()) => {}
            Err(AgentError::Serial(message)) | Err(AgentError::Station(message)) => {
                tracing::warn!(error = %message, "Station capture did not complete; polling for recovery");
                sleep(poll_interval).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Replay ACK after process restart for all durable pending evidence, whether it is still LOCAL
/// or has already advanced to SERVER. A duplicate ACK is safe: the Station accepts it only when
/// capture_id and event_hash match its current WAIT_ACK evidence. Replaying SERVER rows closes the
/// window where the host write succeeded and HTTP acceptance completed but the Station lost the ACK.
pub async fn replay_durable_acks<T: StationTransport>(
    transport: &mut T,
    spool: &Spool,
    expected_station_pubkey33: &[u8; 33],
) -> Result<usize, AgentError> {
    let mut sent = 0usize;
    for row in spool.pending().await? {
        if !matches!(row.state, OutboxState::Local | OutboxState::Server) {
            continue;
        }
        if &row.station_pubkey != expected_station_pubkey33 {
            return Err(AgentError::Contract(
                "durable LOCAL evidence Station public key does not match configured Station"
                    .into(),
            ));
        }
        let ack = AckPayload {
            capture_id: row.capture_id,
            event_hash: row.event_hash,
        };
        transport
            .send(Frame {
                message_type: MessageType::Ack,
                payload: Bytes::copy_from_slice(&encode_ack(&ack)),
            })
            .await?;
        sent += 1;
    }
    Ok(sent)
}

/// Returns true when at least one retryable API failure remains pending.
pub async fn retry_pending_once(
    spool: &Spool,
    api: &ApiClient,
    base_delay: Duration,
) -> Result<bool, AgentError> {
    let mut retryable_failure = false;
    for row in spool.pending().await? {
        let delay = retry_delay(base_delay, row.attempts);
        if !delay.is_zero() {
            sleep(delay).await;
        }

        let result = match row.state {
            OutboxState::Local => api
                .post_evidence(&row)
                .await
                .map(|()| Some(OutboxState::Server)),
            OutboxState::Server => api.evidence_status(&row.event_hash).await.map(|status| {
                if status == EvidenceStatus::Finalized {
                    Some(OutboxState::Finalized)
                } else {
                    None
                }
            }),
            OutboxState::Finalized | OutboxState::Quarantined => Ok(None),
        };

        match result {
            Ok(Some(next_state)) => spool.advance(row.capture_id, next_state).await?,
            Ok(None) => {}
            Err(AgentError::Api(message)) => {
                spool.record_failure(row.capture_id, &message).await?;
                retryable_failure = true;
            }
            Err(AgentError::ApiTerminal(message)) => {
                spool.quarantine(row.capture_id, &message).await?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(retryable_failure)
}

/// Exponential retry delay derived from the durable attempt count. The first upload of a new
/// LOCAL row is immediate; failed retries double from the configured poll interval up to 30 seconds.
pub fn retry_delay(base_delay: Duration, attempts: u32) -> Duration {
    if attempts == 0 || base_delay.is_zero() {
        return Duration::ZERO;
    }
    let base = base_delay.min(MAX_RETRY_BACKOFF);
    let shift = attempts.saturating_sub(1).min(31);
    base.checked_mul(1u32 << shift)
        .unwrap_or(MAX_RETRY_BACKOFF)
        .min(MAX_RETRY_BACKOFF)
}

pub async fn process_capture<T: StationTransport>(
    transport: &mut T,
    spool: &Spool,
    api: &ApiClient,
    expected_station_pubkey33: &[u8; 33],
    station_response_timeout: Duration,
    command: &StationCommand,
) -> Result<(), AgentError> {
    command.validate()?;
    let command_payload = encode_command(command)?;
    transport
        .send(Frame {
            message_type: MessageType::Command,
            payload: Bytes::copy_from_slice(&command_payload),
        })
        .await?;

    let response = timeout(station_response_timeout, transport.receive())
        .await
        .map_err(|_| AgentError::Serial("timed out waiting for Station response".into()))??;
    match response.message_type {
        MessageType::EventReady => {
            let ready = decode_event_ready(&response.payload)?;
            let row = validate_event_ready(command, expected_station_pubkey33, &ready)?;
            spool.persist_local(&row).await?;

            let ack = AckPayload {
                capture_id: row.capture_id,
                event_hash: row.event_hash,
            };
            transport
                .send(Frame {
                    message_type: MessageType::Ack,
                    payload: Bytes::copy_from_slice(&encode_ack(&ack)),
                })
                .await?;

            match api.post_evidence(&row).await {
                Ok(()) => spool.advance(row.capture_id, OutboxState::Server).await,
                Err(AgentError::Api(message)) => {
                    spool.record_failure(row.capture_id, &message).await?;
                    Ok(())
                }
                Err(AgentError::ApiTerminal(message)) => {
                    spool.quarantine(row.capture_id, &message).await
                }
                Err(error) => Err(error),
            }
        }
        MessageType::Error => {
            let station_error = decode_error(&response.payload)?;
            if !station_error.capture_id.is_nil() && station_error.capture_id != command.capture_id
            {
                return Err(AgentError::Contract(
                    "Station ERROR capture_id does not match the active command".into(),
                ));
            }
            Err(AgentError::Station(format!(
                "Station rejected capture with {:?}",
                station_error.code
            )))
        }
        other => Err(AgentError::Contract(format!(
            "unexpected Station response {:?} while capture is active",
            other
        ))),
    }
}

fn validate_event_ready(
    command: &StationCommand,
    expected_station_pubkey33: &[u8; 33],
    ready: &EventReadyPayload,
) -> Result<OutboxRow, AgentError> {
    if ready.capture_id != command.capture_id {
        return Err(AgentError::Contract(
            "EVENT_READY capture_id does not match the active command".into(),
        ));
    }
    if &ready.station_pubkey33 != expected_station_pubkey33 {
        return Err(AgentError::Contract(
            "EVENT_READY Station public key does not match configured Station".into(),
        ));
    }

    let event = StationEvent::decode(&ready.event_bytes)
        .map_err(|error| AgentError::Contract(format!("invalid StationEvent: {error}")))?;
    let station_id = derive_station_id(&ready.station_pubkey33)
        .map_err(|error| AgentError::Contract(format!("invalid Station public key: {error}")))?;
    if event.station_id != station_id {
        return Err(AgentError::Contract(
            "StationEvent station_id does not match the supplied public key".into(),
        ));
    }
    verify_station_signature(
        &ready.event_bytes,
        &ready.station_pubkey33,
        &ready.station_signature64,
    )
    .map_err(|error| AgentError::Contract(format!("invalid Station signature: {error}")))?;

    if hash_canonical_rfid(&ready.observed_rfid) != event.new_rfid_hash {
        return Err(AgentError::Contract(
            "observed RFID hash does not match StationEvent new_rfid_hash".into(),
        ));
    }

    let expected_action = Action::try_from(command.action)
        .map_err(|error| AgentError::Contract(format!("invalid command action: {error}")))?;
    if event.action != expected_action
        || event.deployment_id != command.deployment_id
        || event.animal_id != command.animal_id
        || event.event_sequence != command.event_sequence
        || event.identity_revision != command.identity_revision
        || event.previous_event_hash != command.previous_event_hash
        || event.old_rfid_hash != command.expected_old_rfid_hash
        || event.from_custodian != command.from_custodian
        || event.to_custodian != command.to_custodian
    {
        return Err(AgentError::Contract(
            "StationEvent does not match the immutable command context".into(),
        ));
    }
    if event.action == Action::Transfer && event.new_rfid_hash != command.expected_old_rfid_hash {
        return Err(AgentError::Contract(
            "TRANSFER must preserve the current RFID hash".into(),
        ));
    }

    Ok(OutboxRow {
        capture_id: ready.capture_id,
        event_hash: event.event_hash(),
        event_bytes: ready.event_bytes,
        observed_rfid: ready.observed_rfid,
        station_pubkey: ready.station_pubkey33,
        station_signature: ready.station_signature64,
        state: OutboxState::Local,
        attempts: 0,
    })
}
