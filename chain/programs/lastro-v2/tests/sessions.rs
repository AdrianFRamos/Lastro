//! Handling sessions and batched presence proofs (SPEC v2.1, S3/S4a/S8).

mod common;

use anchor_lang::InstructionData;
use common::*;
use lastro_protocol::{
    crypto::derive_station_id,
    v2::{MAX_BATCH_EVENT_AGE_SECONDS, session_id},
};
use lastro_v2::{
    constants::{SESSION_STATUS_OPEN, SESSION_STATUS_REVOKED},
    state::{BatchAnchor, SessionGrant},
};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

const NOW: i64 = 1_000;
const HOUR: i64 = 3_600;
const FARM: [u8; 32] = [0xfa; 32];

struct Fixture {
    h: Harness,
    station_id: [u8; 32],
    custodian: Keypair,
    sponsor: Keypair,
    /// Id of the last session `open` attempted (derived from custodian and terms).
    session_id: [u8; 32],
}

fn setup() -> Fixture {
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let pubkey33 = compressed_pubkey(&signing_key());
    let station_id = derive_station_id(&pubkey33).unwrap();
    h.svm.expire_blockhash();
    assert_success(send_register_station(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        station_id,
        pubkey33,
    ));
    h.svm.expire_blockhash();
    assert_success(send_register_facility(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        FARM,
        h.authority.pubkey(),
        lastro_v2::constants::FACILITY_TYPE_FARM,
    ));
    h.svm.expire_blockhash();
    assert_success(send_set_station_facility(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        station_id,
        FARM,
    ));
    let sponsor = funded_sponsor(&mut h.svm);
    Fixture {
        h,
        station_id,
        custodian: Keypair::new(),
        sponsor,
        session_id: [0; 32],
    }
}

fn session_pda(f: &Fixture) -> Pubkey {
    Pubkey::find_program_address(
        &[
            lastro_v2::constants::SESSION_SEED,
            &f.h.deployment_id,
            &f.session_id,
        ],
        &program_id(),
    )
    .0
}

fn batch_pda(f: &Fixture, index: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[
            lastro_v2::constants::BATCH_SEED,
            &f.h.deployment_id,
            &f.session_id,
            &index.to_le_bytes(),
        ],
        &program_id(),
    )
    .0
}

fn open(
    f: &mut Fixture,
    max_events: u32,
    valid_from: i64,
    expires_at: i64,
    actions: u16,
) -> TestResult {
    f.session_id = session_id(
        &f.h.deployment_id,
        &f.custodian.pubkey().to_bytes(),
        &f.station_id,
        valid_from,
        expires_at,
        max_events,
    );
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(f.custodian.pubkey(), true),
            AccountMeta::new(f.sponsor.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_registry_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_pda(&f.h.deployment_id, &f.station_id).0, false),
            AccountMeta::new_readonly(
                facility_pda(&f.h.deployment_id, &station_facility(f)).0,
                false,
            ),
            AccountMeta::new(session_pda(f), false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::OpenSession {
            session_id: f.session_id,
            station_id: f.station_id,
            allowed_actions: actions,
            max_events,
            valid_from,
            expires_at,
        }
        .data(),
    };
    f.h.svm.expire_blockhash();
    let custodian = f.custodian.insecure_clone();
    send_sponsored(&mut f.h.svm, vec![ix], &f.sponsor, &[&custodian])
}

/// The facility the Station is currently bound to (all zero when unbound).
fn station_facility(f: &Fixture) -> [u8; 32] {
    account::<lastro_v2::state::StationRecord>(
        &f.h.svm,
        &station_pda(&f.h.deployment_id, &f.station_id).0,
    )
    .facility_id
}

fn open_default(f: &mut Fixture) {
    assert_success(open(f, 1_000, NOW, NOW + 4 * HOUR, 1));
}

fn anchor(f: &mut Fixture, relayer: &Keypair, leaves: u32, first: i64, last: i64) -> TestResult {
    let index = account::<SessionGrant>(&f.h.svm, &session_pda(f)).batch_count;
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(relayer.pubkey(), true),
            AccountMeta::new(f.sponsor.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new(session_pda(f), false),
            AccountMeta::new_readonly(station_pda(&f.h.deployment_id, &f.station_id).0, false),
            AccountMeta::new(batch_pda(f, index), false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::AnchorObservationBatch {
            merkle_root: [0x77; 32],
            leaf_count: leaves,
            first_observed_at: first,
            last_observed_at: last,
        }
        .data(),
    };
    f.h.svm.expire_blockhash();
    send_sponsored(&mut f.h.svm, vec![ix], &f.sponsor, &[relayer])
}

fn revoke(f: &mut Fixture, signer: &Keypair) -> TestResult {
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(signer.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new(session_pda(f), false),
        ],
        data: lastro_v2::instruction::RevokeSession {}.data(),
    };
    f.h.svm.expire_blockhash();
    send_sponsored(&mut f.h.svm, vec![ix], &f.sponsor, &[signer])
}

fn close(f: &mut Fixture, caller: &Keypair, rent_recipient: Pubkey) -> TestResult {
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(caller.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new(session_pda(f), false),
            AccountMeta::new(rent_recipient, false),
        ],
        data: lastro_v2::instruction::CloseSession {}.data(),
    };
    f.h.svm.expire_blockhash();
    send_sponsored(&mut f.h.svm, vec![ix], &f.sponsor, &[caller])
}

#[test]
fn one_signature_opens_a_session_and_batches_anchor_hundreds_of_observations() {
    // PURPOSE: S3/S4a — 500 presence proofs cost one custodian signature to open and one
    // account per batch, not one per animal; the custodian holds no lamports.
    // ARRANGE: registered Station, zero-balance custodian, funded sponsor.
    let mut f = setup();
    open_default(&mut f);
    let session: SessionGrant = account(&f.h.svm, &session_pda(&f));
    assert_eq!(session.status, SESSION_STATUS_OPEN);
    assert_eq!(session.custodian, f.custodian.pubkey());
    assert_eq!(session.rent_payer, f.sponsor.pubkey());

    // ACTION: two batches (300 + 200 leaves) anchored by the custodian an hour in.
    set_unix_timestamp(&mut f.h.svm, NOW + HOUR);
    let custodian = f.custodian.insecure_clone();
    assert_success(anchor(&mut f, &custodian, 300, NOW + 10, NOW + 1_800));
    assert_success(anchor(&mut f, &custodian, 200, NOW + 1_800, NOW + 3_000));

    // ASSERT: counters advanced, each batch keeps the session terms the verifier needs.
    let session: SessionGrant = account(&f.h.svm, &session_pda(&f));
    assert_eq!((session.events_used, session.batch_count), (500, 2));
    let second: BatchAnchor = account(&f.h.svm, &batch_pda(&f, 1));
    assert_eq!(second.session_id, f.session_id);
    assert_eq!(second.batch_index, 1);
    assert_eq!(second.custodian, f.custodian.pubkey());
    assert_eq!(second.station_id, f.station_id);
    assert_eq!(second.leaf_count, 200);
    assert_eq!(second.anchored_at, NOW + HOUR);
    assert_eq!(second.facility_id, FARM);
    assert_eq!(
        account::<SessionGrant>(&f.h.svm, &session_pda(&f)).facility_id,
        FARM
    );
    assert_eq!(f.h.svm.get_balance(&f.custodian.pubkey()).unwrap_or(0), 0);
    // FAILURE MEANS: a corral session would still need per-animal signatures or accounts.
}

#[test]
fn session_terms_are_bounded_at_opening() {
    // PURPOSE: a session authorizes only presence proofs, for at most 12 h and 2 000 events,
    // starting now, on a Station valid for the whole window.
    let mut f = setup();
    for (max_events, from, until, actions) in [
        (0, NOW, NOW + HOUR, 1),           // no capacity
        (2_001, NOW, NOW + HOUR, 1),       // above MAX_SESSION_EVENTS
        (10, NOW, NOW + 12 * HOUR + 1, 1), // longer than 12 h
        (10, NOW - 601, NOW + HOUR, 1),    // backdated authorization
        (10, NOW, NOW + HOUR, 2),          // BIND is not a session action in v2.1
        (10, NOW, NOW + HOUR, 3),
        (10, NOW, 86_401, 1), // outlives the Station's registration (valid_until = 86 400)
    ] {
        assert_failure(open(&mut f, max_events, from, until, actions));
    }
    assert_success(open(&mut f, 2_000, NOW, NOW + 12 * HOUR, 1));
    // FAILURE MEANS: one signature could authorize unbounded or retroactive evidence.
}

#[test]
fn batches_must_fit_the_session_window_capacity_and_relayer() {
    // PURPOSE: the program enforces every session term it can see without the leaves.
    let mut f = setup();
    assert_success(open(&mut f, 100, NOW, NOW + 2 * HOUR, 1));
    set_unix_timestamp(&mut f.h.svm, NOW + HOUR);
    let custodian = f.custodian.insecure_clone();
    let stranger = funded_sponsor(&mut f.h.svm);
    let authority = f.h.authority.insecure_clone();

    assert_failure(anchor(&mut f, &stranger, 1, NOW + 10, NOW + 20)); // not custodian/authority
    assert_failure(anchor(&mut f, &custodian, 0, NOW + 10, NOW + 20)); // empty
    assert_failure(anchor(&mut f, &custodian, 101, NOW + 10, NOW + 20)); // over capacity
    assert_failure(anchor(&mut f, &custodian, 1, NOW - 1, NOW + 20)); // before the window
    assert_failure(anchor(&mut f, &custodian, 1, NOW + 30, NOW + 20)); // inverted
    assert_failure(anchor(&mut f, &custodian, 1, NOW + 10, NOW + HOUR + 1)); // from the future
    // The deployment authority may relay on the custodian's behalf.
    assert_success(anchor(&mut f, &authority, 60, NOW + 10, NOW + 20));
    assert_failure(anchor(&mut f, &custodian, 41, NOW + 30, NOW + 40)); // 60 + 41 > 100
    assert_success(anchor(&mut f, &custodian, 40, NOW + 30, NOW + 40));
    // FAILURE MEANS: a session could anchor evidence outside the window it authorized.
}

#[test]
fn offline_observations_anchor_within_seven_days_and_never_after_revocation() {
    // PURPOSE: S8 — a corral without signal anchors later, but only what was observed while
    // the session was valid, and only within the offline window.
    let mut f = setup();
    open_default(&mut f);
    let custodian = f.custodian.insecure_clone();
    let last = NOW + 3 * HOUR;

    // Six days after the last observation (session long expired): still anchorable.
    set_unix_timestamp(&mut f.h.svm, last + 6 * 86_400);
    assert_success(anchor(&mut f, &custodian, 10, NOW + 60, last));
    // Past seven days: refused.
    set_unix_timestamp(&mut f.h.svm, last + MAX_BATCH_EVENT_AGE_SECONDS + 1);
    assert_failure(anchor(&mut f, &custodian, 10, NOW + 60, last));

    // Revocation: observations after it are refused, earlier ones stay anchorable.
    let mut f = setup();
    open_default(&mut f);
    let custodian = f.custodian.insecure_clone();
    set_unix_timestamp(&mut f.h.svm, NOW + HOUR);
    let stranger = funded_sponsor(&mut f.h.svm);
    assert_failure(revoke(&mut f, &stranger));
    assert_success(revoke(&mut f, &custodian));
    let session: SessionGrant = account(&f.h.svm, &session_pda(&f));
    assert_eq!(session.status, SESSION_STATUS_REVOKED);
    assert_eq!(session.revoked_at, NOW + HOUR);
    set_unix_timestamp(&mut f.h.svm, NOW + 2 * HOUR);
    assert_failure(anchor(&mut f, &custodian, 5, NOW + 60, NOW + HOUR + 1));
    assert_success(anchor(&mut f, &custodian, 5, NOW + 60, NOW + HOUR));
    // FAILURE MEANS: a revoked session or a stale offline queue could still anchor evidence.
}

#[test]
fn only_the_custodian_closes_early_and_the_rent_returns_to_its_payer() {
    // PURPOSE: the grant's rent is recoverable, and nobody else can end a live session.
    let mut f = setup();
    open_default(&mut f);
    let custodian = f.custodian.insecure_clone();
    let stranger = funded_sponsor(&mut f.h.svm);
    let sponsor = f.sponsor.pubkey();
    assert_failure(close(&mut f, &stranger, sponsor));
    assert_failure(close(&mut f, &custodian, stranger.pubkey())); // refund redirected
    let before = f.h.svm.get_balance(&sponsor).unwrap();
    assert_success(close(&mut f, &custodian, sponsor));
    assert!(!account_exists(&f.h.svm, &session_pda(&f)));
    let rent =
        f.h.svm
            .minimum_balance_for_rent_exemption(SessionGrant::SPACE);
    // The sponsor also paid this transaction's fees (2 signatures).
    assert_eq!(
        f.h.svm.get_balance(&sponsor).unwrap(),
        before + rent - 10_000
    );

    // After the offline window anyone may close an abandoned grant.
    let mut f = setup();
    open_default(&mut f);
    let stranger = funded_sponsor(&mut f.h.svm);
    let sponsor = f.sponsor.pubkey();
    set_unix_timestamp(
        &mut f.h.svm,
        NOW + 4 * HOUR + MAX_BATCH_EVENT_AGE_SECONDS + 1,
    );
    assert_success(close(&mut f, &stranger, sponsor));
    // FAILURE MEANS: grants would lock rent forever or could be closed under the custodian.
}

#[test]
fn a_session_requires_a_station_installed_at_an_active_facility() {
    // PURPOSE: S5 — every batch is attributable to the facility its Station was installed at.
    // ARRANGE: a Station registered but never installed; then installed at a suspended farm.
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let pubkey33 = compressed_pubkey(&signing_key());
    let station_id = derive_station_id(&pubkey33).unwrap();
    h.svm.expire_blockhash();
    assert_success(send_register_station(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        station_id,
        pubkey33,
    ));
    let sponsor = funded_sponsor(&mut h.svm);
    let mut f = Fixture {
        h,
        station_id,
        custodian: Keypair::new(),
        sponsor,
        session_id: [0; 32],
    };
    // Unbound Station: the facility PDA of [0; 32] does not exist, opening fails.
    assert_failure(open(&mut f, 10, NOW, NOW + HOUR, 1));

    // Only the deployment authority installs a Station, and only at a registered facility.
    let stranger = funded_sponsor(&mut f.h.svm);
    f.h.svm.expire_blockhash();
    assert_success(send_register_facility(
        &mut f.h.svm,
        &f.h.authority.insecure_clone(),
        f.h.deployment_id,
        FARM,
        f.h.authority.pubkey(),
        lastro_v2::constants::FACILITY_TYPE_FARM,
    ));
    f.h.svm.expire_blockhash();
    assert_failure(send_set_station_facility(
        &mut f.h.svm,
        &stranger,
        f.h.deployment_id,
        station_id,
        FARM,
    ));
    assert_eq!(station_facility(&f), [0; 32]);
    let authority = f.h.authority.insecure_clone();
    f.h.svm.expire_blockhash();
    assert_success(send_set_station_facility(
        &mut f.h.svm,
        &authority,
        f.h.deployment_id,
        station_id,
        FARM,
    ));
    assert_eq!(station_facility(&f), FARM);
    // A window past the facility's validity (valid_until = 86 400) is refused.
    assert_failure(open(&mut f, 10, NOW, 86_401, 1));
    assert_success(open(&mut f, 10, NOW, NOW + HOUR, 1));
    // FAILURE MEANS: presence proofs could be anchored with no facility to locate them.
}

#[test]
fn a_session_id_cannot_be_squatted_by_another_wallet() {
    // PURPOSE: M1 — the id binds the custodian, so a stranger cannot open it first and take
    // over the presence proofs the custodian's Station signs for it.
    let mut f = setup();
    let victim = f.custodian.insecure_clone();
    let expires_at = NOW + HOUR;
    let victim_id = session_id(
        &f.h.deployment_id,
        &victim.pubkey().to_bytes(),
        &f.station_id,
        NOW,
        expires_at,
        10,
    );

    // ACTION: a stranger submits open_session with the victim's id and its own signature.
    let stranger = Keypair::new();
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(stranger.pubkey(), true),
            AccountMeta::new(f.sponsor.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_registry_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_pda(&f.h.deployment_id, &f.station_id).0, false),
            AccountMeta::new_readonly(facility_pda(&f.h.deployment_id, &FARM).0, false),
            AccountMeta::new(
                Pubkey::find_program_address(
                    &[
                        lastro_v2::constants::SESSION_SEED,
                        &f.h.deployment_id,
                        &victim_id,
                    ],
                    &program_id(),
                )
                .0,
                false,
            ),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::OpenSession {
            session_id: victim_id,
            station_id: f.station_id,
            allowed_actions: 1,
            max_events: 10,
            valid_from: NOW,
            expires_at,
        }
        .data(),
    };
    f.h.svm.expire_blockhash();
    let sponsor = f.sponsor.insecure_clone();
    assert_failure(send_sponsored(
        &mut f.h.svm,
        vec![ix],
        &sponsor,
        &[&stranger],
    ));

    // ASSERT: the custodian still opens its own session with that id.
    assert_success(open(&mut f, 10, NOW, expires_at, 1));
    assert_eq!(f.session_id, victim_id);
    // FAILURE MEANS: any wallet could occupy a custodian's session and bind its proofs.
}

#[test]
fn a_suspended_station_anchors_nothing_more() {
    // PURPOSE: M3 — once the authority suspends or revokes a Station (a leaked key), proofs
    // still waiting offline can no longer be anchored under it.
    let mut f = setup();
    open_default(&mut f);
    let custodian = f.custodian.insecure_clone();
    set_unix_timestamp(&mut f.h.svm, NOW + 60);
    assert_success(anchor(&mut f, &custodian, 10, NOW + 10, NOW + 20));

    // ACTION: the authority suspends the Station.
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(f.h.authority.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_registry_pda(&f.h.deployment_id).0, false),
            AccountMeta::new(station_pda(&f.h.deployment_id, &f.station_id).0, false),
        ],
        data: lastro_v2::instruction::SetStationStatus {
            status: lastro_v2::constants::STATION_STATUS_SUSPENDED,
        }
        .data(),
    };
    f.h.svm.expire_blockhash();
    let authority = f.h.authority.insecure_clone();
    assert_success(send_instructions(&mut f.h.svm, vec![ix], &authority));

    // ASSERT: the next batch, even of proofs signed before the suspension, is refused.
    assert_failure(anchor(&mut f, &custodian, 10, NOW + 30, NOW + 40));
    // FAILURE MEANS: evidence signed with a compromised Station key could still be anchored.
}

fn set_station_status(f: &mut Fixture, status: u8) -> TestResult {
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(f.h.authority.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&f.h.deployment_id).0, false),
            AccountMeta::new_readonly(station_registry_pda(&f.h.deployment_id).0, false),
            AccountMeta::new(station_pda(&f.h.deployment_id, &f.station_id).0, false),
        ],
        data: lastro_v2::instruction::SetStationStatus { status }.data(),
    };
    f.h.svm.expire_blockhash();
    let authority = f.h.authority.insecure_clone();
    send_instructions(&mut f.h.svm, vec![ix], &authority)
}

#[test]
fn an_expired_station_returns_only_through_a_validity_extension() {
    // PURPOSE: L1 — EXPIRED -> SUSPENDED -> ACTIVE must not revive an expired key; extending
    // the validity window is the one deliberate path back.
    use lastro_v2::constants::{
        STATION_STATUS_ACTIVE, STATION_STATUS_EXPIRED, STATION_STATUS_SUSPENDED,
    };
    let mut f = setup();
    assert_success(set_station_status(&mut f, STATION_STATUS_EXPIRED));
    assert_failure(set_station_status(&mut f, STATION_STATUS_SUSPENDED));
    assert_failure(set_station_status(&mut f, STATION_STATUS_ACTIVE));

    f.h.svm.expire_blockhash();
    let authority = f.h.authority.insecure_clone();
    assert_success(send_extend_station_validity(
        &mut f.h.svm,
        &authority,
        f.h.deployment_id,
        f.station_id,
        NOW + 400 * 86_400,
    ));
    let station = account::<lastro_v2::state::StationRecord>(
        &f.h.svm,
        &station_pda(&f.h.deployment_id, &f.station_id).0,
    );
    assert_eq!(station.status, STATION_STATUS_ACTIVE);
    // FAILURE MEANS: an expired Station key could be reactivated without a new validity term.
}
