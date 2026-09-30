//! v2 physical identity contracts: RFID binding, reidentification and non-reuse.

mod common;

use anchor_lang::InstructionData;
use common::*;
use lastro_protocol::{
    crypto::derive_station_id,
    v2::{DomainEventEnvelope, EventType, identifier_payload_hash},
};
use lastro_v2::state::{AssetState, RfidBinding};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

const NOW: i64 = 1_000;
const EVENT_OFFSET: u16 = 8 + 32 + 32 + 32;

struct Station {
    key: SigningKey,
    pubkey33: [u8; 33],
    id: [u8; 32],
}

fn station(h: &mut Harness) -> Station {
    let key = signing_key();
    let pubkey33 = compressed_pubkey(&key);
    let id = derive_station_id(&pubkey33).expect("station id");
    assert_success(send_register_station(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        id,
        pubkey33,
    ));
    Station { key, pubkey33, id }
}

fn rfid_pda(h: &Harness, rfid: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[b"rfid", &h.deployment_id, rfid], &program_id()).0
}

fn event_pda(h: &Harness, event_id: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[b"event", &h.deployment_id, event_id], &program_id()).0
}

fn envelope(
    h: &Harness,
    station: &Station,
    event_type: EventType,
    asset_id: [u8; 32],
    event_id: [u8; 32],
    old: [u8; 32],
    new: [u8; 32],
) -> [u8; 220] {
    let asset: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &asset_id).0);
    DomainEventEnvelope::new(
        event_type,
        h.deployment_id,
        asset_id,
        event_id,
        asset.state_version + 1,
        asset.last_event_hash,
        identifier_payload_hash(old, new),
        station.id,
        NOW,
        NOW + 60,
    )
    .expect("valid envelope")
    .encode()
    .expect("encode")
}

fn secp(station: &Station, event: &[u8; 220]) -> Instruction {
    let signature: Signature = station.key.sign(event);
    let signature = signature.normalize_s().unwrap_or(signature);
    let mut data = vec![0u8; 113];
    data[0] = 1;
    for (index, value) in [16u16, 0, 80, 0, EVENT_OFFSET, 220, 1].iter().enumerate() {
        data[2 + index * 2..4 + index * 2].copy_from_slice(&value.to_le_bytes());
    }
    data[16..80].copy_from_slice(&signature.to_bytes());
    data[80..113].copy_from_slice(&station.pubkey33);
    Instruction {
        program_id: Pubkey::new_from_array(solana_sdk_ids_secp256r1()),
        accounts: vec![],
        data,
    }
}

fn solana_sdk_ids_secp256r1() -> [u8; 32] {
    Pubkey::from_str_const("Secp256r1SigVerify1111111111111111111111111").to_bytes()
}

#[allow(clippy::too_many_arguments)]
fn send_identity(
    h: &mut Harness,
    custodian: &Keypair,
    station: &Station,
    asset_id: [u8; 32],
    event_id: [u8; 32],
    old: Option<[u8; 32]>,
    new: [u8; 32],
    signed_payload: ([u8; 32], [u8; 32]),
) -> TestResult {
    let event_type = if old.is_some() {
        EventType::IdentifierReplaced
    } else {
        EventType::IdentifierBound
    };
    let event = envelope(
        h,
        station,
        event_type,
        asset_id,
        event_id,
        signed_payload.0,
        signed_payload.1,
    );
    let mut accounts = vec![
        AccountMeta::new(custodian.pubkey(), true),
        AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
        AccountMeta::new_readonly(station_registry_pda(&h.deployment_id).0, false),
        AccountMeta::new_readonly(station_pda(&h.deployment_id, &station.id).0, false),
        AccountMeta::new(asset_pda(&h.deployment_id, &asset_id).0, false),
        AccountMeta::new(event_pda(h, &event_id), false),
    ];
    let data = match old {
        Some(old) => {
            accounts.push(AccountMeta::new(rfid_pda(h, &old), false));
            accounts.push(AccountMeta::new(rfid_pda(h, &new), false));
            lastro_v2::instruction::ReplaceIdentifier {
                subject_id: asset_id,
                event_id,
                station_id: station.id,
                event,
                old_rfid_hash: old,
                new_rfid_hash: new,
            }
            .data()
        }
        None => {
            accounts.push(AccountMeta::new(rfid_pda(h, &new), false));
            lastro_v2::instruction::BindIdentifier {
                subject_id: asset_id,
                event_id,
                station_id: station.id,
                event,
                new_rfid_hash: new,
            }
            .data()
        }
    };
    accounts.push(AccountMeta::new_readonly(
        Pubkey::from_str_const("Sysvar1nstructions1111111111111111111111111"),
        false,
    ));
    accounts.push(AccountMeta::new_readonly(system_program_id(), false));
    let lastro = Instruction {
        program_id: program_id(),
        accounts,
        data,
    };
    h.svm.expire_blockhash();
    send_instructions(&mut h.svm, vec![secp(station, &event), lastro], custodian)
}

fn setup() -> (Harness, Station, Keypair) {
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    h.svm.expire_blockhash();
    let station = station(&mut h);
    let custodian = h.other.insecure_clone();
    for asset_id in [[0xa1; 32], [0xa2; 32]] {
        h.svm.expire_blockhash();
        assert_success(send_register_asset(
            &mut h.svm,
            &h.authority,
            h.deployment_id,
            asset_id,
            custodian.pubkey(),
            500_000,
        ));
    }
    (h, station, custodian)
}

const ASSET: [u8; 32] = [0xa1; 32];
const OTHER_ASSET: [u8; 32] = [0xa2; 32];
const TAG_1: [u8; 32] = [0x11; 32];
const TAG_2: [u8; 32] = [0x12; 32];
const ZERO: [u8; 32] = [0; 32];

#[test]
fn tag_changes_but_the_asset_identity_does_not() {
    // PURPOSE: the core Lastro guarantee on v2 — reidentification keeps the AssetID.
    // ARRANGE: asset custodied by `other`, registered Station.
    let (mut h, station, custodian) = setup();
    // ACTION: bind TAG_1, then replace it with TAG_2.
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [1; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [2; 32],
        Some(TAG_1),
        TAG_2,
        (TAG_1, TAG_2),
    ));
    // ASSERT: same AssetState, active TAG_2, TAG_1 retired, event chain advanced twice.
    let asset: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &ASSET).0);
    assert_eq!(asset.current_rfid_hash, TAG_2);
    assert_eq!(asset.state_version, 2);
    let old: RfidBinding = account(&h.svm, &rfid_pda(&h, &TAG_1));
    let new: RfidBinding = account(&h.svm, &rfid_pda(&h, &TAG_2));
    assert_eq!((old.asset_id, old.status), (ASSET, 2));
    assert_eq!((new.asset_id, new.status), (ASSET, 1));
    // FAILURE MEANS: losing a tag would fork or lose the animal's history.
}

#[test]
fn an_rfid_can_never_identify_a_second_asset() {
    // PURPOSE: retired or active tags cannot be reused by another animal.
    let (mut h, station, custodian) = setup();
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [1; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    // ASSERT: binding the active TAG_1 to another asset fails.
    assert_failure(send_identity(
        &mut h,
        &custodian,
        &station,
        OTHER_ASSET,
        [2; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [3; 32],
        Some(TAG_1),
        TAG_2,
        (TAG_1, TAG_2),
    ));
    // ASSERT: the retired TAG_1 cannot be bound to another asset either.
    assert_failure(send_identity(
        &mut h,
        &custodian,
        &station,
        OTHER_ASSET,
        [4; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    // FAILURE MEANS: a recycled tag could graft one animal's history onto another.
}

#[test]
fn only_the_custodian_with_a_station_signed_payload_can_change_identity() {
    // PURPOSE: identity changes need both custody authority and physical Station evidence.
    let (mut h, station, custodian) = setup();
    let intruder = h.authority.insecure_clone();
    // ASSERT: a non-custodian signer is rejected.
    assert_failure(send_identity(
        &mut h,
        &intruder,
        &station,
        ASSET,
        [1; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    // ASSERT: an envelope whose signed payload names a different tag is rejected.
    assert_failure(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [2; 32],
        None,
        TAG_1,
        (ZERO, TAG_2),
    ));
    // ASSERT: a first binding cannot be sent as a replacement of a tag the asset never had.
    assert_failure(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [3; 32],
        Some(TAG_2),
        TAG_1,
        (TAG_2, TAG_1),
    ));
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [4; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    // ASSERT: a second first-binding is rejected once a tag is active.
    assert_failure(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [5; 32],
        None,
        TAG_2,
        (ZERO, TAG_2),
    ));
    // FAILURE MEANS: an operator or a forged payload could rewrite an animal's identity.
}

fn send_observation(
    h: &mut Harness,
    signer: &Keypair,
    station: &Station,
    event_id: [u8; 32],
    tag: [u8; 32],
) -> TestResult {
    let event = envelope(
        h,
        station,
        EventType::ObservationRecorded,
        ASSET,
        event_id,
        tag,
        tag,
    );
    let lastro = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(station_registry_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(station_pda(&h.deployment_id, &station.id).0, false),
            AccountMeta::new(asset_pda(&h.deployment_id, &ASSET).0, false),
            AccountMeta::new(event_pda(h, &event_id), false),
            AccountMeta::new_readonly(
                Pubkey::from_str_const("Sysvar1nstructions1111111111111111111111111"),
                false,
            ),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::RecordObservation {
            subject_id: ASSET,
            event_id,
            station_id: station.id,
            event,
        }
        .data(),
    };
    h.svm.expire_blockhash();
    send_instructions(&mut h.svm, vec![secp(station, &event), lastro], signer)
}

#[test]
fn presence_proofs_are_signed_by_the_custodian_or_the_authority_only() {
    // PURPOSE: day-to-day presence proofs must not require the deployment authority key,
    //          yet a stranger must not be able to anchor them.
    // ARRANGE: asset custodied by `other`, bound to TAG_1; a funded stranger wallet.
    let (mut h, station, custodian) = setup();
    assert_success(send_identity(
        &mut h,
        &custodian,
        &station,
        ASSET,
        [1; 32],
        None,
        TAG_1,
        (ZERO, TAG_1),
    ));
    let stranger = Keypair::new();
    h.svm
        .airdrop(&stranger.pubkey(), LAMPORTS_PER_TEST_WALLET)
        .expect("airdrop");
    // ACTION: stranger, then custodian, then authority anchor presence proofs of TAG_1.
    assert_failure(send_observation(
        &mut h, &stranger, &station, [2; 32], TAG_1,
    ));
    assert_success(send_observation(
        &mut h, &custodian, &station, [3; 32], TAG_1,
    ));
    let authority = h.authority.insecure_clone();
    assert_success(send_observation(
        &mut h, &authority, &station, [4; 32], TAG_1,
    ));
    // ASSERT: only the two authorized proofs advanced the asset.
    let asset: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &ASSET).0);
    assert_eq!(asset.state_version, 3);
    // FAILURE MEANS: presence evidence could be forged by anyone, or needs a hot authority key.
}
