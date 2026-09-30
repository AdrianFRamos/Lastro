//! Custody-transfer, authority-rotation and proven-lineage transformation contracts.

mod common;

use common::*;
use lastro_protocol::v2::{
    LineageLeaf, LineageRole, MassBalance, TransformationManifest, custody_transfer_payload_hash,
    merkle_proof, merkle_root,
};
use lastro_v2::{
    constants::{
        ASSET_STATUS_ACTIVE, ASSET_STATUS_CONSUMED, FACILITY_TYPE_PROCESSING_FACILITY,
        INTENT_STATUS_CONSUMED, INTENT_TYPE_CUSTODY_TRANSFER, TRANSFORMATION_STATUS_FINALIZED,
    },
    state::{AssetState, IntentState, ProtocolConfigV2, TransformationAnchor},
};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use anchor_lang::InstructionData;

const NOW: i64 = 1_000;

fn fund(h: &mut Harness, wallet: &Keypair) {
    h.svm
        .airdrop(&wallet.pubkey(), LAMPORTS_PER_TEST_WALLET)
        .expect("airdrop");
}

fn propose_transfer(
    h: &mut Harness,
    asset_id: [u8; 32],
    intent_id: [u8; 32],
    to: Pubkey,
    version: u64,
) -> TestResult {
    let payload =
        custody_transfer_payload_hash(h.deployment_id, asset_id, to.to_bytes(), version, 7);
    send_create_typed_intent(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        asset_id,
        intent_id,
        INTENT_TYPE_CUSTODY_TRANSFER,
        version,
        7,
        NOW + 600,
        payload,
    )
}

#[test]
fn custody_transfer_requires_the_named_receiver_to_sign_and_is_one_shot() {
    // PURPOSE: custody moves only when the named recipient accepts a live proposal.
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let asset_id = [0xa1; 32];
    let intent_id = [0xa2; 32];
    let intruder = Keypair::new();
    fund(&mut h, &intruder);

    assert_success(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        asset_id,
        h.authority.pubkey(),
        500_000,
    ));
    h.svm.expire_blockhash();
    let receiver = h.other.pubkey();
    assert_success(propose_transfer(&mut h, asset_id, intent_id, receiver, 0));

    // A wallet other than the one committed in the payload cannot take custody.
    h.svm.expire_blockhash();
    assert_failure(send_accept_custody_transfer(
        &mut h.svm,
        &intruder,
        h.deployment_id,
        asset_id,
        intent_id,
        0,
    ));

    h.svm.expire_blockhash();
    assert_success(send_accept_custody_transfer(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        asset_id,
        intent_id,
        0,
    ));
    let asset: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &asset_id).0);
    assert_eq!(asset.custodian, h.other.pubkey());
    assert_eq!(asset.state_version, 1);
    let intent: IntentState = account(
        &h.svm,
        &intent_pda(&h.deployment_id, &asset_id, &intent_id).0,
    );
    assert_eq!(intent.status, INTENT_STATUS_CONSUMED);

    // Replay of the consumed proposal fails, and the former custodian lost authority.
    h.svm.expire_blockhash();
    assert_failure(send_accept_custody_transfer(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        asset_id,
        intent_id,
        0,
    ));
    h.svm.expire_blockhash();
    assert_failure(propose_transfer(
        &mut h,
        asset_id,
        [0xa3; 32],
        intruder.pubkey(),
        1,
    ));
}

#[test]
fn stale_custody_proposal_is_rejected_after_state_changes() {
    // PURPOSE: a proposal is bound to the asset state_version it was created for.
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let asset_id = [0xb1; 32];
    assert_success(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        asset_id,
        h.authority.pubkey(),
        500_000,
    ));
    h.svm.expire_blockhash();
    let receiver = h.other.pubkey();
    assert_success(propose_transfer(&mut h, asset_id, [0xb2; 32], receiver, 0));
    h.svm.expire_blockhash();
    assert_success(propose_transfer(&mut h, asset_id, [0xb3; 32], receiver, 0));

    h.svm.expire_blockhash();
    assert_success(send_accept_custody_transfer(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        asset_id,
        [0xb2; 32],
        0,
    ));
    // The second proposal targeted state_version 0 and the proposer no longer holds custody.
    h.svm.expire_blockhash();
    assert_failure(send_accept_custody_transfer(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        asset_id,
        [0xb3; 32],
        0,
    ));
}

#[test]
fn config_authority_rotation_needs_both_signatures() {
    // PURPOSE: the deployment authority can only be rotated by old and new keys together.
    let mut h = Harness::new();
    h.initialize();
    let successor = Keypair::new();
    fund(&mut h, &successor);

    assert_success(send_transfer_config_authority(
        &mut h.svm,
        &h.authority,
        &successor,
        h.deployment_id,
    ));
    let config: ProtocolConfigV2 = account(&h.svm, &config_pda(&h.deployment_id).0);
    assert_eq!(config.authority, successor.pubkey());

    h.svm.expire_blockhash();
    assert_failure(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        [0xc1; 32],
        h.authority.pubkey(),
        1_000,
    ));
    h.svm.expire_blockhash();
    assert_success(send_register_asset(
        &mut h.svm,
        &successor,
        h.deployment_id,
        [0xc1; 32],
        successor.pubkey(),
        1_000,
    ));
}

struct Plan {
    transformation_id: [u8; 32],
    facility_id: [u8; 32],
    inputs: Vec<LineageLeaf>,
    outputs: Vec<LineageLeaf>,
    manifest: TransformationManifest,
}

fn leaf(asset_id: [u8; 32], role: LineageRole, position: u32, weight: u64) -> LineageLeaf {
    LineageLeaf {
        asset_id,
        role,
        position,
        quantity: 1,
        weight_grams: weight,
    }
}

fn plan() -> Plan {
    let transformation_id = [0xe1; 32];
    let facility_id = [0xe2; 32];
    let inputs = vec![
        leaf([0x11; 32], LineageRole::Input, 0, 300_000),
        leaf([0x12; 32], LineageRole::Input, 1, 200_000),
    ];
    let outputs = vec![
        leaf([0x21; 32], LineageRole::Output, 0, 250_000),
        leaf([0x22; 32], LineageRole::Output, 1, 150_000),
        leaf([0x23; 32], LineageRole::Byproduct, 2, 80_000),
    ];
    let manifest = TransformationManifest {
        transformation_id,
        facility_id,
        transformation_type: 1,
        input_root: merkle_root(&inputs).unwrap(),
        output_root: merkle_root(&outputs).unwrap(),
        input_count: 2,
        output_count: 3,
        mass: MassBalance {
            input_weight_grams: 500_000,
            output_weight_grams: 400_000,
            byproduct_weight_grams: 80_000,
            loss_weight_grams: 20_000,
            tolerance_basis_points: 100,
        },
        manifest_nonce: 1,
        expires_at: NOW + 3_600,
    };
    Plan {
        transformation_id,
        facility_id,
        inputs,
        outputs,
        manifest,
    }
}

fn begin(h: &mut Harness, operator: &Keypair, plan: &Plan) -> TestResult {
    let m = &plan.manifest;
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(operator.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(facility_registry_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(facility_pda(&h.deployment_id, &plan.facility_id).0, false),
            AccountMeta::new(
                transformation_pda(&h.deployment_id, &plan.transformation_id).0,
                false,
            ),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::BeginTransformation {
            transformation_id: m.transformation_id,
            facility_id: m.facility_id,
            transformation_type: m.transformation_type,
            input_root: m.input_root,
            output_root: m.output_root,
            input_count: m.input_count,
            output_count: m.output_count,
            input_weight_grams: m.mass.input_weight_grams,
            output_weight_grams: m.mass.output_weight_grams,
            byproduct_weight_grams: m.mass.byproduct_weight_grams,
            loss_weight_grams: m.mass.loss_weight_grams,
            tolerance_basis_points: m.mass.tolerance_basis_points,
            manifest_nonce: m.manifest_nonce,
            expires_at: m.expires_at,
            manifest_hash: m.manifest_hash().unwrap(),
        }
        .data(),
    };
    send_instructions(&mut h.svm, vec![ix], operator)
}

fn reserve(
    h: &mut Harness,
    operator: &Keypair,
    plan: &Plan,
    index: usize,
    proof: Vec<[u8; 32]>,
) -> TestResult {
    let input = plan.inputs[index];
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(operator.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(facility_registry_pda(&h.deployment_id).0, false),
            AccountMeta::new(
                transformation_pda(&h.deployment_id, &plan.transformation_id).0,
                false,
            ),
            AccountMeta::new_readonly(facility_pda(&h.deployment_id, &plan.facility_id).0, false),
            AccountMeta::new(asset_pda(&h.deployment_id, &input.asset_id).0, false),
            AccountMeta::new(
                reservation_pda(&h.deployment_id, &plan.transformation_id, &input.asset_id).0,
                false,
            ),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::ReserveTransformationInput {
            transformation_id: plan.transformation_id,
            asset_id: input.asset_id,
            weight_grams: input.weight_grams,
            expected_state_version: 0,
            leaf_position: input.position,
            leaf_quantity: input.quantity,
            leaf_index: index as u32,
            proof,
        }
        .data(),
    };
    send_instructions(&mut h.svm, vec![ix], operator)
}

fn consume(h: &mut Harness, operator: &Keypair, plan: &Plan, index: usize) -> TestResult {
    let asset_id = plan.inputs[index].asset_id;
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(operator.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new(
                transformation_pda(&h.deployment_id, &plan.transformation_id).0,
                false,
            ),
            AccountMeta::new_readonly(facility_pda(&h.deployment_id, &plan.facility_id).0, false),
            AccountMeta::new(asset_pda(&h.deployment_id, &asset_id).0, false),
            AccountMeta::new(
                reservation_pda(&h.deployment_id, &plan.transformation_id, &asset_id).0,
                false,
            ),
        ],
        data: lastro_v2::instruction::ConsumeTransformationInput {
            transformation_id: plan.transformation_id,
            asset_id,
            expected_state_version: 0,
        }
        .data(),
    };
    send_instructions(&mut h.svm, vec![ix], operator)
}

fn create_output(h: &mut Harness, operator: &Keypair, plan: &Plan, index: usize) -> TestResult {
    let output = plan.outputs[index];
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(operator.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new(
                transformation_pda(&h.deployment_id, &plan.transformation_id).0,
                false,
            ),
            AccountMeta::new_readonly(facility_pda(&h.deployment_id, &plan.facility_id).0, false),
            AccountMeta::new(asset_pda(&h.deployment_id, &output.asset_id).0, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::CreateTransformationOutput {
            output_id: output.asset_id,
            asset_type: 4,
            lineage_root: plan.manifest.input_root,
            weight_grams: output.weight_grams,
            leaf_role: output.role as u8,
            leaf_position: output.position,
            leaf_quantity: output.quantity,
            leaf_index: index as u32,
            proof: merkle_proof(&plan.outputs, index).unwrap(),
        }
        .data(),
    };
    send_instructions(&mut h.svm, vec![ix], operator)
}

fn finalize(h: &mut Harness, operator: &Keypair, plan: &Plan) -> TestResult {
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(operator.pubkey(), true),
            AccountMeta::new_readonly(config_pda(&h.deployment_id).0, false),
            AccountMeta::new_readonly(facility_pda(&h.deployment_id, &plan.facility_id).0, false),
            AccountMeta::new(
                transformation_pda(&h.deployment_id, &plan.transformation_id).0,
                false,
            ),
        ],
        data: lastro_v2::instruction::FinalizeTransformation {}.data(),
    };
    send_instructions(&mut h.svm, vec![ix], operator)
}

#[test]
fn facility_owner_runs_a_proven_transformation_end_to_end() {
    // PURPOSE: transformations consume proven inputs and create proven outputs.
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let operator = Keypair::new();
    fund(&mut h, &operator);
    let plan = plan();

    assert_success(send_register_facility(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        plan.facility_id,
        operator.pubkey(),
        FACILITY_TYPE_PROCESSING_FACILITY,
    ));
    for input in &plan.inputs {
        h.svm.expire_blockhash();
        assert_success(send_register_asset(
            &mut h.svm,
            &h.authority,
            h.deployment_id,
            input.asset_id,
            operator.pubkey(),
            input.weight_grams,
        ));
    }

    // The deployment authority is not the facility owner and cannot operate it.
    h.svm.expire_blockhash();
    let authority = h.authority.insecure_clone();
    assert_failure(begin(&mut h, &authority, &plan));
    h.svm.expire_blockhash();
    assert_success(begin(&mut h, &operator, &plan));

    // A proof for a different leaf index does not open the input.
    h.svm.expire_blockhash();
    let wrong = merkle_proof(&plan.inputs, 1).unwrap();
    assert_failure(reserve(&mut h, &operator, &plan, 0, wrong));

    // Every manifest input is reserved while the transformation is OPEN; the first consumption
    // moves it to FINALIZING, after which no new reservation is accepted.
    for index in 0..plan.inputs.len() {
        h.svm.expire_blockhash();
        let proof = merkle_proof(&plan.inputs, index).unwrap();
        assert_success(reserve(&mut h, &operator, &plan, index, proof));
    }
    for index in 0..plan.inputs.len() {
        h.svm.expire_blockhash();
        assert_success(consume(&mut h, &operator, &plan, index));
        let input: AssetState = account(
            &h.svm,
            &asset_pda(&h.deployment_id, &plan.inputs[index].asset_id).0,
        );
        assert_eq!(input.status, ASSET_STATUS_CONSUMED);
    }

    // The consumed reservation remains, so the same input cannot be reserved twice.
    h.svm.expire_blockhash();
    let proof = merkle_proof(&plan.inputs, 0).unwrap();
    assert_failure(reserve(&mut h, &operator, &plan, 0, proof));

    for index in 0..plan.outputs.len() {
        h.svm.expire_blockhash();
        assert_success(create_output(&mut h, &operator, &plan, index));
        let output: AssetState = account(
            &h.svm,
            &asset_pda(&h.deployment_id, &plan.outputs[index].asset_id).0,
        );
        assert_eq!(output.custodian, operator.pubkey());
        assert_eq!(output.status, ASSET_STATUS_ACTIVE);
        assert_eq!(output.parent_root, plan.manifest.input_root);
    }

    h.svm.expire_blockhash();
    assert_success(finalize(&mut h, &operator, &plan));
    let transformation: TransformationAnchor = account(
        &h.svm,
        &transformation_pda(&h.deployment_id, &plan.transformation_id).0,
    );
    assert_eq!(transformation.status, TRANSFORMATION_STATUS_FINALIZED);
    assert_eq!(transformation.consumed_input_weight_grams, 500_000);
    assert_eq!(transformation.created_output_weight_grams, 400_000);
    assert_eq!(transformation.created_byproduct_weight_grams, 80_000);
}

#[test]
fn inputs_outside_the_operator_custody_cannot_be_reserved() {
    // PURPOSE: a facility may only transform assets it holds in custody.
    let mut h = Harness::new();
    h.initialize();
    set_unix_timestamp(&mut h.svm, NOW);
    let operator = Keypair::new();
    fund(&mut h, &operator);
    let plan = plan();

    assert_success(send_register_facility(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        plan.facility_id,
        operator.pubkey(),
        FACILITY_TYPE_PROCESSING_FACILITY,
    ));
    h.svm.expire_blockhash();
    // Input 0 still belongs to another custodian.
    assert_success(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        plan.inputs[0].asset_id,
        h.other.pubkey(),
        plan.inputs[0].weight_grams,
    ));
    h.svm.expire_blockhash();
    assert_success(begin(&mut h, &operator, &plan));
    h.svm.expire_blockhash();
    let proof = merkle_proof(&plan.inputs, 0).unwrap();
    assert_failure(reserve(&mut h, &operator, &plan, 0, proof));
}
