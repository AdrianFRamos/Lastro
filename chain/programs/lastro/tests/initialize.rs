//! LiteSVM integration contracts for deployment initialization.

mod common;

use common::*;
use solana_signer::Signer;

#[test]
fn initialize_creates_config_with_deployment_and_station() {
    // PURPOSE: Persist exactly the deployment and authorized Station.
    // ARRANGE: Clean LiteSVM; authority signer; deployment_id D; valid compressed pubkey33 K.
    let mut h = Harness::new();
    let (_, expected_bump) = protocol_config_pda(&h.deployment_id);
    // ACTION: Submit initialize for PDA ["config",D].
    h.initialize();
    let config = protocol_config(&h.svm, &h.deployment_id);
    // ASSERT: ProtocolConfig is program-owned and contains the exact authority, D, K, and expected bump.
    assert_eq!(config.authority.to_bytes(), h.authority.pubkey().to_bytes());
    assert_eq!(config.deployment_id, h.deployment_id);
    assert_eq!(config.station_pubkey33, h.station_pubkey33);
    assert_eq!(config.bump, expected_bump);
    // FAILURE MEANS: A deployment could start with a Station identity different from configuration.
}

#[test]
fn initialize_rejects_duplicate_config_pda() {
    // PURPOSE: A deployment config must be created only once.
    // ARRANGE: Execute a valid initialize for D.
    let mut h = Harness::new();
    h.initialize();
    let (config_address, _) = protocol_config_pda(&h.deployment_id);
    let before = account_data(&h.svm, &config_address).expect("config must exist");
    h.svm.expire_blockhash();
    // ACTION: Execute initialize again for the same PDA.
    let result = send_initialize(&mut h.svm, &h.authority, h.deployment_id, h.station_pubkey33);
    // ASSERT: The second transaction fails and the first account is unchanged.
    assert_failure(result);
    assert_eq!(account_data(&h.svm, &config_address).unwrap(), before);
    // FAILURE MEANS: Authority could silently replace the Station within the same deployment.
}

#[test]
fn initialize_rejects_invalid_station_pubkey_encoding() {
    // PURPOSE: The [u8;33] type fixes length; the program must still reject invalid SEC1 encoding.
    // ARRANGE: Invalid prefix and an out-of-field compressed x-coordinate, each in a fresh deployment.
    let invalid_prefix = [0x04; 33];
    let mut invalid_point = [0u8; 33];
    invalid_point[0] = 0x02;
    invalid_point[1..].copy_from_slice(&[
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ]);

    for (deployment, key) in [([0x41; 32], invalid_prefix), ([0x42; 32], invalid_point)] {
        let mut h = Harness::new();
        // ACTION: Call initialize for each invalid fixture.
        let result = send_initialize(&mut h.svm, &h.authority, deployment, key);
        // ASSERT: Each invalid input fails without creating ProtocolConfig.
        assert_failure_contains(result, "InvalidEvent");
        let (config, _) = protocol_config_pda(&deployment);
        assert!(account_data(&h.svm, &config).is_none());
    }

    let mut valid = Harness::new();
    let result = send_initialize(
        &mut valid.svm,
        &valid.authority,
        valid.deployment_id,
        valid.station_pubkey33,
    );
    assert_success(result);
    // FAILURE MEANS: The program could register a key that cannot be used by the Secp256r1 precompile.
}
