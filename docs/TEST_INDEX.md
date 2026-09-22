# Complete Test Index

This file is generated from the repository's real test declarations. Test files are the source of truth; this index supports coverage review and navigation.

**Declared cases:** 593  
**Files containing test cases:** 117

Ignored or environment-gated cases are not proven by declaration alone; only an executed non-skipped run counts as validation evidence.

## `apps/web/e2e/demo.spec.ts`

- L16: `full ORIGIN -> A->B -> REIDENTIFY -> B->C flow preserves one AnimalID`
- L105: `stale Wallet A attack after A->B is rejected and cannot change canonical state`

## `apps/web/e2e/failure_recovery.spec.ts`

- L16: `lost API response followed by Agent retry does not create duplicate evidence or transitions`
- L42: `page reload reconstructs UI from durable projection/evidence rather than browser memory`
- L76: `wallet rejection leaves accepted physical evidence non-finalized and canonical state unchanged`
- L112: `reload after wallet broadcast but before SUBMITTED registration reuses the same transaction signature`
- L150: `reload after durable SUBMITTED state resumes finalization without a second wallet signature`

## `apps/web/e2e/layout-16x9.spec.ts`

- L75: `landing keeps all three primary paths inside the first 1600x900 viewport`

## `apps/web/e2e/reidentify.spec.ts`

- L16: `visual recovery resolves the existing AnimalID before a new RFID capture`
- L53: `retired RFID A is no longer presented or resolved as current after REIDENTIFY to B`
- L84: `missing both RFID and visual recovery identifier leaves identity UNRESOLVED`

## `apps/web/e2e/storytelling.spec.ts`

- L10: `landing, problem and future routes render the intended product narrative`
- L51: `presentation pages do not overflow horizontally on the mobile target viewport`

## `apps/web/e2e/verifier.spec.ts`

- L15: `exported package from a real completed flow verifies all five layers`
- L46: `one-byte mutation of signed event data makes the verifier explicitly INVALID`
- L72: `local-file verification continues when the Lastro evidence API is unavailable`

## `apps/web/tests/api/client.test.ts`

- L34: `createAnimal sends only the visual recovery identifier required by hackathon scope`
- L48: `maps every non-success API response to an explicit typed client error`
- L64: `preserves evidence package payload exactly for the verifier layer`
- L75: `network timeout or abort never produces an optimistic success result`
- L91: `parses submitted transaction metadata separately from capture status`
- L112: `submits only the transaction signature to the confirmed-verification endpoint`
- L134: `rejects inconsistent capture event lifecycle metadata`
- L160: `rejects non-submitted statuses from the transaction submission endpoint`
- L175: `rejects malformed or oversized durable identifiers`

## `apps/web/tests/components/AnimalState.test.ts`

- L22: `renders every canonical/projection field required by the demo state contract`
- L36: `renders pre-ORIGIN absence explicitly without inventing canonical values`

## `apps/web/tests/components/CustodyTimeline.test.ts`

- L12: `preserves every event including REIDENTIFY in canonical sequence order`
- L33: `does not collapse distinct events that have duplicate-looking labels`

## `apps/web/tests/components/StationPanel.test.ts`

- L12: `renders only hardware facts actually represented by the hackathon Station contract`
- L27: `renders unknown Station facts explicitly rather than defaulting to ready`

## `apps/web/tests/components/VerificationPanel.test.ts`

- L21: `renders all five verification layers independently`
- L34: `keeps the invalid layer and concrete failure reason visible`

## `apps/web/tests/config.test.ts`

- L29: `rejects every missing required VITE deployment variable independently`
- L42: `rejects malformed or unsupported API and RPC URL schemes`
- L62: `requires an explicit Wallet Standard solana chain identifier`
- L78: `normalizes only API trailing slash and preserves deployment identity values`
- L93: `contains only public deployment values and never exposes server or private-key secrets`

## `apps/web/tests/demo/pendingOperation.test.ts`

- L32: `round-trips the pending operation exactly and clears it explicitly`
- L46: `fails closed and removes malformed local storage`
- L58: `rejects malformed or oversized durable identifiers from local storage`

## `apps/web/tests/pages/DemoPage.test.ts`

- L145: `enables ORIGIN only for an unoriginated animal with a connected intended custodian wallet`
- L166: `normal TRANSFER action is available only to the current custodian wallet`
- L184: `REIDENTIFY uses visual ID for recovery but still requires current custodian authority`
- L203: `exposes the stale-custodian attack only in the explicit stale-authority demo state`
- L222: `wallet signature rejection leaves canonical and projected state unchanged`
- L257: `does not display fabricated canonical success when API projection is unavailable`
- L276: `restores selected animal and timeline from durable sources after page reload`
- L296: `connects the explicitly selected Wallet Standard wallet`
- L320: `reports physical identity continuity as unresolved when no identifier resolves an animal`
- L336: `reload resumes a durable SUBMITTED transaction without signing again`
- L385: `reload re-verifies a locally remembered broadcast signature without signing a second transaction`

## `apps/web/tests/pages/StoryRoutes.test.ts`

- L19: `exposes the intended public information architecture`
- L36: `renders the landing paths with Demo as the primary action`
- L54: `renders the sourced physical trust gap story`
- L73: `separates proven functionality from the future expansion path`

## `apps/web/tests/pages/VerifyPage.test.ts`

- L72: `accepts a local evidence package and runs verification without the Lastro evidence API`
- L94: `initial verification layers are NOT_CHECKED rather than optimistic VALID`
- L110: `malformed or unsupported packages can never reach a canonical VALID result`
- L139: `shows RPC unavailability separately from local evidence validity`

## `apps/web/tests/protocol/evidence.test.ts`

- L12: `accepts the strict frozen EvidencePackage fixture without rewriting proof bytes`
- L24: `rejects unsupported evidence package versions`
- L36: `rejects malformed encodings missing fields wrong lengths and undeclared verdict fields`
- L60: `round trips evidence transport without changing event order or signed bytes`
- L70: `rejects malformed transaction signatures`

## `apps/web/tests/protocol/hash.test.ts`

- L14: `rfid hash matches frozen cross-language vectors for both physical tags`
- L25: `station id matches the frozen compressed-key vector`
- L35: `event hash matches every frozen 276-byte StationEvent vector`
- L47: `does not hash reader text or framing as if it were canonical RFID bytes`

## `apps/web/tests/protocol/p256.test.ts`

- L31: `verifies every frozen low-S P-256 fixture over its exact raw StationEvent`
- L50: `rejects a signature when one StationEvent byte changes`
- L62: `rejects an otherwise valid signature under a different Station public key`
- L74: `rejects malformed compressed-key and compact-signature encodings`
- L89: `rejects the high-S equivalent of a valid signature`
- L102: `requires raw 276-byte event input and cannot verify by passing event_hash as message`
- L115: `maps malformed off-curve public keys to false instead of throwing`

## `apps/web/tests/protocol/stationEvent.test.ts`

- L13: `decodes the complete 276-byte ORIGIN fixture field by field`
- L31: `decodes sequence and revision with the protocol little-endian convention`
- L47: `rejects wrong magic version action and non-zero reserved bytes`
- L61: `rejects any StationEvent length other than exactly 276 bytes`
- L73: `rejects action-specific semantic combinations that cannot represent a valid transition`

## `apps/web/tests/solana/transaction.test.ts`

- L94: `rejects transaction data targeting a program id different from configured Lastro`
- L106: `requires Secp256r1 verification immediately before the bound Lastro instruction`
- L121: `preserves validated raw instruction bytes without reserializing StationEvent data`
- L135: `rejects Secp256r1 offsets that do not reference the exact serialized StationEvent`
- L149: `rejects measured transaction size above 1232 bytes`
- L165: `requires connected wallet to equal the transition authority encoded by current state`
- L185: `requires connected wallet to support the requested transaction version`

## `apps/web/tests/solana/wallet.test.ts`

- L35: `connects through Wallet Standard discovery using the configured Solana client integration`
- L53: `reports missing wallet explicitly and never fabricates a connected authority`
- L65: `never requests stores or logs wallet private key material`
- L96: `enumerates public wallet choices and connects one explicitly by name`
- L116: `never falls back to another wallet when an explicit wallet name is missing`

## `apps/web/tests/verify/chain.test.ts`

- L341: `checks canonical accounts but does not mark transactionless evidence as final valid proof`
- L353: `rejects a package whose terminal custodian differs from canonical RPC state`
- L365: `rejects every terminal RFID revision sequence or last-event-hash mismatch`
- L381: `represents RPC unavailability as NOT_CHECKED and never as canonical validity`
- L397: `requires the AnimalState account owner and binary layout to match the Lastro program`
- L425: `rejects a mixed set of present and missing transaction signatures`
- L444: `binds every txSignature to the exact finalized Lastro transaction envelope`
- L461: `rejects non-finalized and failed evidence transactions`
- L481: `rejects a finalized transaction whose signature differs from EvidencePackage`
- L500: `rejects tampered StationEvent bytes program or accounts in finalized transactions`
- L536: `times out stalled RPC as NOT_CHECKED`

## `apps/web/tests/verify/package.test.ts`

- L23: `valid frozen package passes every local verification layer before RPC`
- L36: `one changed signed event byte makes the package invalid`
- L47: `changed observed RFID value breaks the event-to-physical-evidence binding`
- L60: `omitting a middle event is detected by sequence and predecessor validation`
- L73: `reordering individually valid signed events invalidates the history`
- L86: `detects a fork even when each individual Station signature is valid`
- L99: `rejects later use of the retired RFID after REIDENTIFY`

## `chain/programs/lastro/tests/account_constraints.rs`

- L8: `animal_state_pda_uses_deployment_and_animal_id`
- L26: `rfid_binding_pda_uses_deployment_and_rfid_hash`
- L44: `protocol_config_pda_uses_deployment_id`
- L61: `wrong_pda_accounts_are_rejected`

## `chain/programs/lastro/tests/initialize.rs`

- L9: `initialize_creates_config_with_deployment_and_station`
- L26: `initialize_rejects_duplicate_config_pda`
- L43: `initialize_rejects_invalid_station_pubkey_encoding`

## `chain/programs/lastro/tests/origin.rs`

- L10: `origin_creates_animal_state_and_active_rfid_binding`
- L34: `origin_requires_sequence_one_revision_one_zero_predecessor`
- L61: `origin_requires_to_custodian_signer`
- L79: `origin_rejects_existing_animal_state`
- L100: `origin_rejects_rfid_ever_bound_to_another_history`

## `chain/programs/lastro/tests/reidentify.rs`

- L19: `reidentify_preserves_animal_id_and_custodian`
- L37: `reidentify_increments_revision_and_sequence_once`
- L55: `reidentify_retires_old_binding_and_activates_new`
- L73: `reidentify_rejects_same_new_rfid`
- L92: `reidentify_rejects_new_rfid_already_known`
- L118: `reidentify_requires_current_custodian_signer`

## `chain/programs/lastro/tests/rfid_binding.rs`

- L9: `only_one_active_binding_can_point_to_an_rfid`
- L30: `retired_binding_keeps_original_animal_id`
- L47: `retired_rfid_cannot_be_reoriginated`
- L66: `lookup_active_binding_matches_animal_state_current_rfid`

## `chain/programs/lastro/tests/secp_binding.rs`

- L9: `valid_secp_over_exact_event_is_accepted`
- L24: `missing_secp_instruction_fails`
- L40: `wrong_station_pubkey_fails`
- L59: `signature_and_public_key_descriptor_offsets_and_indexes_are_exact`
- L90: `signature_over_other_276_bytes_fails`
- L109: `message_offset_one_byte_wrong_fails`
- L129: `message_length_275_or_277_fails`
- L149: `wrong_instruction_index_fails`
- L167: `secp_after_lastro_fails_when_program_requires_expected_order`
- L184: `high_s_signature_is_rejected_by_runtime_precompile`
- L203: `extra_instruction_fails_frozen_two_instruction_envelope`

## `chain/programs/lastro/tests/state_machine.rs`

- L10: `full_origin_transfer_reidentify_transfer_reaches_expected_terminal_state`
- L31: `replay_of_any_consumed_event_fails`
- L63: `stale_revision_after_reidentify_fails`
- L82: `old_rfid_after_reidentify_fails`
- L103: `fork_with_valid_station_signature_but_wrong_predecessor_fails`

## `chain/programs/lastro/tests/transaction_size.rs`

- L17: `origin_serialized_transaction_fits_selected_format_limit`
- L31: `transfer_serialized_transaction_fits_selected_format_limit`
- L45: `reidentify_serialized_transaction_fits_selected_format_limit`
- L59: `size_test_includes_wallet_signature_accounts_and_both_instructions`

## `chain/programs/lastro/tests/transfer.rs`

- L19: `transfer_a_to_b_updates_only_custodian_sequence_hash`
- L37: `old_custodian_cannot_transfer_after_a_to_b`
- L63: `transfer_requires_physical_rfid_equal_current_binding`
- L84: `transfer_rejects_sequence_gap`
- L99: `transfer_rejects_wrong_predecessor`
- L115: `transfer_rejects_revision_change`
- L132: `transfer_rejects_same_destination_as_current_custodian`

## `crates/lastro-protocol/tests/decode_rejection.rs`

- L8: `rejects_275_bytes`
- L16: `rejects_277_bytes`
- L26: `rejects_wrong_magic`
- L36: `rejects_unknown_version`
- L46: `rejects_unknown_action`
- L56: `rejects_nonzero_reserved`
- L66: `arbitrary_bytes_never_panic_and_non_exact_lengths_never_decode`

## `crates/lastro-protocol/tests/evidence_package.rs`

- L18: `package_preserves_event_order`
- L32: `package_rejects_wrong_event_length`
- L44: `package_rejects_wrong_key_signature_lengths`
- L57: `package_does_not_trust_valid_flag`
- L67: `package_detects_one_byte_evidence_tampering`
- L77: `package_rejects_noncanonical_signature_text`

## `crates/lastro-protocol/tests/ids.rs`

- L8: `animal_id_is_exactly_32_bytes`
- L16: `station_id_matches_domain_separated_pubkey_hash`
- L27: `invalid_p256_pubkey_length_is_rejected`

## `crates/lastro-protocol/tests/layout.rs`

- L8: `station_event_is_exactly_276_bytes`
- L16: `all_offsets_are_contiguous_and_end_at_276`
- L38: `header_is_lstr_version_one_and_reserved_zero`
- L50: `integers_are_little_endian`

## `crates/lastro-protocol/tests/origin.rs`

- L14: `origin_requires_sequence_one`
- L24: `origin_requires_revision_one`
- L34: `origin_requires_zero_predecessor_and_old_rfid`
- L47: `origin_requires_zero_from_and_nonzero_to`

## `crates/lastro-protocol/tests/p256.rs`

- L8: `valid_compact_low_s_signature_verifies_raw_event`
- L20: `one_byte_event_change_breaks_signature`
- L33: `wrong_station_key_breaks_signature`
- L45: `high_s_signature_is_rejected`

## `crates/lastro-protocol/tests/reidentify.rs`

- L8: `reidentify_requires_new_rfid`
- L18: `reidentify_keeps_custodian`
- L28: `reidentify_advances_revision_exactly_once`

## `crates/lastro-protocol/tests/rfid.rs`

- L11: `same_canonical_rfid_hashes_identically`
- L20: `different_canonical_rfid_values_hash_differently`
- L30: `rfid_hash_uses_exact_domain_separator`
- L43: `formatted_display_text_is_not_silently_canonical`
- L51: `canonical_rfid_is_exactly_8_bytes`

## `crates/lastro-protocol/tests/transfer.rs`

- L8: `transfer_keeps_same_rfid`
- L18: `transfer_keeps_revision`
- L31: `transfer_rejects_self_destination`

## `crates/lastro-protocol/tests/vectors.rs`

- L17: `origin_fixture_matches_reference`
- L25: `transfer_fixture_matches_reference`
- L33: `reidentify_fixture_matches_reference`

## `firmware/station/components/lastro_station/test/test_event.c`

- L14: `event_origin_matches_fixture_byte_for_byte`
- L24: `event_transfer_matches_fixture_byte_for_byte`
- L34: `event_reidentify_matches_fixture_byte_for_byte`
- L44: `event_integers_are_little_endian`
- L63: `event_reserved_is_always_zero`
- L77: `event_rejects_invalid_origin_semantics`
- L90: `event_rejects_invalid_transfer_semantics`
- L103: `event_rejects_invalid_reidentify_semantics`
- L116: `event_sha256_matches_rust_fixture`

## `firmware/station/components/lastro_station/test/test_rfid.c`

- L5: `rfid_valid_frame_yields_one_canonical_id`
- L15: `rfid_fragmented_frame_yields_same_id`
- L25: `rfid_noise_outside_valid_frame_does_not_emit_id`
- L35: `rfid_truncated_frame_emits_nothing`
- L45: `rfid_reader_integrity_error_is_rejected`
- L55: `rfid_two_consecutive_tags_are_not_merged`
- L65: `rfid_hash_matches_rust_vector`
- L82: `rfid_canonical_value_is_exactly_8_bytes`

## `firmware/station/components/lastro_station/test/test_runtime.c`

- L50: `runtime consumes Agent COMMAND and emits signed EVENT_READY after physical observation`
- L89: `runtime retries an EVENT_READY frame after a transient Station-to-Agent write failure`
- L127: `runtime rejects inbound Station-only message types`

## `firmware/station/components/lastro_station/test/test_signer.c`

- L63: `signer rejects messages that are not exactly 276 bytes`
- L74: `development signer exposes the frozen compressed Station public key`
- L88: `development signature verifies as P-256 SHA-256 over the raw StationEvent`
- L129: `signer emits canonical compact low-S signatures`
- L147: `eFuse signer preserves the Station protocol contract`

## `firmware/station/components/lastro_station/test/test_station.c`

- L28: `station boots idle`
- L37: `station command moves to wait RFID`
- L49: `station does not sign without valid RFID`
- L63: `station rejects second command while busy`
- L80: `station new RFID hash comes from observed RFID`
- L99: `station ACK returns to idle only for matching event`
- L124: `station invalid transfer observation returns to safe state`
- L149: `station accepts an identical command replay while waiting for RFID`
- L165: `station replays identical signed evidence when the same command returns before ACK`

## `firmware/station/components/lastro_station/test/test_transport.c`

- L18: `command_payload_fixture_matches_224_byte_wire_contract`
- L49: `event_ready_payload_fixture_matches_397_byte_wire_contract`
- L72: `ack_and_error_payload_offsets_are_exact`
- L101: `crc32c_castagnoli_known_vector_is_e3069283`
- L112: `transport_roundtrip_command_and_event_ready`
- L139: `transport_handles_every_single_byte_chunking`
- L161: `transport_bad_crc_is_rejected_and_next_good_frame_recovers`
- L186: `transport_oversized_payload_is_rejected_before_copy`
- L213: `transport exposes back-to-back frames from one UART chunk in order`

## `firmware/station/components/lastro_station/test/test_vectors.c`

- L14: `c_origin_vector_equals_repository_fixture`
- L24: `c_transfer_vector_equals_repository_fixture`
- L34: `c_reidentify_vector_equals_repository_fixture`

## `firmware/station/pytest/test_hardware.py`

- L229: `test_g1_real_rfid_to_p256_to_host_verification`
- L245: `test_reader_two_physical_tags_produce_distinct_canonical_ids`
- L278: `test_reboot_preserves_station_public_key`
- L306: `test_h1_efuse_private_key_is_not_readable_by_firmware`

## `hardware-simulator/tests/test_protocol.py`

- L53: `test_crc32c_matches_frozen_castagnoli_vector`
- L57: `test_command_frame_round_trips_at_every_fragment_boundary`
- L69: `test_decoder_rejects_bad_crc_and_recovers_following_frame`
- L85: `test_origin_station_event_matches_cross_language_fixture_bytes`
- L107: `test_transfer_station_event_matches_cross_language_fixture_bytes`
- L135: `test_reidentify_station_event_matches_cross_language_fixture_bytes`

## `hardware-simulator/tests/test_repository_boundary.py`

- L8: `test_core_runtime_source_does_not_depend_on_hardware_simulator`
- L25: `test_simulator_source_stops_at_station_agent_boundary`
- L40: `test_compose_keeps_simulator_optional_and_wire_private`

## `hardware-simulator/tests/test_server.py`

- L66: `test_real_http_and_wire_servers_complete_one_origin_station_capture`

## `hardware-simulator/tests/test_station.py`

- L55: `test_station_origin_flow_emits_valid_low_s_event_and_accepts_matching_ack`
- L88: `test_observation_without_active_capture_is_ignored`
- L96: `test_fault_injection_reader_failure_fails_closed_and_returns_to_idle`
- L110: `test_corrupt_next_event_ready_crc_is_one_shot`
- L126: `test_default_signer_matches_frozen_repository_station_identity`
- L138: `test_identical_command_replay_matches_firmware_wait_states`
- L156: `test_busy_fault_emits_busy_without_consuming_a_capture`
- L167: `test_signing_failure_emits_error_and_does_not_leave_unsigned_evidence_pending`

## `node_modules/@lastro/web/e2e/demo.spec.ts`

- L16: `full ORIGIN -> A->B -> REIDENTIFY -> B->C flow preserves one AnimalID`
- L105: `stale Wallet A attack after A->B is rejected and cannot change canonical state`

## `node_modules/@lastro/web/e2e/failure_recovery.spec.ts`

- L16: `lost API response followed by Agent retry does not create duplicate evidence or transitions`
- L42: `page reload reconstructs UI from durable projection/evidence rather than browser memory`
- L76: `wallet rejection leaves accepted physical evidence non-finalized and canonical state unchanged`
- L112: `reload after wallet broadcast but before SUBMITTED registration reuses the same transaction signature`
- L150: `reload after durable SUBMITTED state resumes finalization without a second wallet signature`

## `node_modules/@lastro/web/e2e/layout-16x9.spec.ts`

- L75: `landing keeps all three primary paths inside the first 1600x900 viewport`

## `node_modules/@lastro/web/e2e/reidentify.spec.ts`

- L16: `visual recovery resolves the existing AnimalID before a new RFID capture`
- L53: `retired RFID A is no longer presented or resolved as current after REIDENTIFY to B`
- L84: `missing both RFID and visual recovery identifier leaves identity UNRESOLVED`

## `node_modules/@lastro/web/e2e/storytelling.spec.ts`

- L10: `landing, problem and future routes render the intended product narrative`
- L51: `presentation pages do not overflow horizontally on the mobile target viewport`

## `node_modules/@lastro/web/e2e/verifier.spec.ts`

- L15: `exported package from a real completed flow verifies all five layers`
- L46: `one-byte mutation of signed event data makes the verifier explicitly INVALID`
- L72: `local-file verification continues when the Lastro evidence API is unavailable`

## `node_modules/@lastro/web/tests/api/client.test.ts`

- L34: `createAnimal sends only the visual recovery identifier required by hackathon scope`
- L48: `maps every non-success API response to an explicit typed client error`
- L64: `preserves evidence package payload exactly for the verifier layer`
- L75: `network timeout or abort never produces an optimistic success result`
- L91: `parses submitted transaction metadata separately from capture status`
- L112: `submits only the transaction signature to the confirmed-verification endpoint`
- L134: `rejects inconsistent capture event lifecycle metadata`
- L160: `rejects non-submitted statuses from the transaction submission endpoint`
- L175: `rejects malformed or oversized durable identifiers`

## `node_modules/@lastro/web/tests/components/AnimalState.test.ts`

- L22: `renders every canonical/projection field required by the demo state contract`
- L36: `renders pre-ORIGIN absence explicitly without inventing canonical values`

## `node_modules/@lastro/web/tests/components/CustodyTimeline.test.ts`

- L12: `preserves every event including REIDENTIFY in canonical sequence order`
- L33: `does not collapse distinct events that have duplicate-looking labels`

## `node_modules/@lastro/web/tests/components/StationPanel.test.ts`

- L12: `renders only hardware facts actually represented by the hackathon Station contract`
- L27: `renders unknown Station facts explicitly rather than defaulting to ready`

## `node_modules/@lastro/web/tests/components/VerificationPanel.test.ts`

- L21: `renders all five verification layers independently`
- L34: `keeps the invalid layer and concrete failure reason visible`

## `node_modules/@lastro/web/tests/config.test.ts`

- L29: `rejects every missing required VITE deployment variable independently`
- L42: `rejects malformed or unsupported API and RPC URL schemes`
- L62: `requires an explicit Wallet Standard solana chain identifier`
- L78: `normalizes only API trailing slash and preserves deployment identity values`
- L93: `contains only public deployment values and never exposes server or private-key secrets`

## `node_modules/@lastro/web/tests/demo/pendingOperation.test.ts`

- L32: `round-trips the pending operation exactly and clears it explicitly`
- L46: `fails closed and removes malformed local storage`
- L58: `rejects malformed or oversized durable identifiers from local storage`

## `node_modules/@lastro/web/tests/pages/DemoPage.test.ts`

- L145: `enables ORIGIN only for an unoriginated animal with a connected intended custodian wallet`
- L166: `normal TRANSFER action is available only to the current custodian wallet`
- L184: `REIDENTIFY uses visual ID for recovery but still requires current custodian authority`
- L203: `exposes the stale-custodian attack only in the explicit stale-authority demo state`
- L222: `wallet signature rejection leaves canonical and projected state unchanged`
- L257: `does not display fabricated canonical success when API projection is unavailable`
- L276: `restores selected animal and timeline from durable sources after page reload`
- L296: `connects the explicitly selected Wallet Standard wallet`
- L320: `reports physical identity continuity as unresolved when no identifier resolves an animal`
- L336: `reload resumes a durable SUBMITTED transaction without signing again`
- L385: `reload re-verifies a locally remembered broadcast signature without signing a second transaction`

## `node_modules/@lastro/web/tests/pages/StoryRoutes.test.ts`

- L19: `exposes the intended public information architecture`
- L36: `renders the landing paths with Demo as the primary action`
- L54: `renders the sourced physical trust gap story`
- L73: `separates proven functionality from the future expansion path`

## `node_modules/@lastro/web/tests/pages/VerifyPage.test.ts`

- L72: `accepts a local evidence package and runs verification without the Lastro evidence API`
- L94: `initial verification layers are NOT_CHECKED rather than optimistic VALID`
- L110: `malformed or unsupported packages can never reach a canonical VALID result`
- L139: `shows RPC unavailability separately from local evidence validity`

## `node_modules/@lastro/web/tests/protocol/evidence.test.ts`

- L12: `accepts the strict frozen EvidencePackage fixture without rewriting proof bytes`
- L24: `rejects unsupported evidence package versions`
- L36: `rejects malformed encodings missing fields wrong lengths and undeclared verdict fields`
- L60: `round trips evidence transport without changing event order or signed bytes`
- L70: `rejects malformed transaction signatures`

## `node_modules/@lastro/web/tests/protocol/hash.test.ts`

- L14: `rfid hash matches frozen cross-language vectors for both physical tags`
- L25: `station id matches the frozen compressed-key vector`
- L35: `event hash matches every frozen 276-byte StationEvent vector`
- L47: `does not hash reader text or framing as if it were canonical RFID bytes`

## `node_modules/@lastro/web/tests/protocol/p256.test.ts`

- L31: `verifies every frozen low-S P-256 fixture over its exact raw StationEvent`
- L50: `rejects a signature when one StationEvent byte changes`
- L62: `rejects an otherwise valid signature under a different Station public key`
- L74: `rejects malformed compressed-key and compact-signature encodings`
- L89: `rejects the high-S equivalent of a valid signature`
- L102: `requires raw 276-byte event input and cannot verify by passing event_hash as message`
- L115: `maps malformed off-curve public keys to false instead of throwing`

## `node_modules/@lastro/web/tests/protocol/stationEvent.test.ts`

- L13: `decodes the complete 276-byte ORIGIN fixture field by field`
- L31: `decodes sequence and revision with the protocol little-endian convention`
- L47: `rejects wrong magic version action and non-zero reserved bytes`
- L61: `rejects any StationEvent length other than exactly 276 bytes`
- L73: `rejects action-specific semantic combinations that cannot represent a valid transition`

## `node_modules/@lastro/web/tests/solana/transaction.test.ts`

- L94: `rejects transaction data targeting a program id different from configured Lastro`
- L106: `requires Secp256r1 verification immediately before the bound Lastro instruction`
- L121: `preserves validated raw instruction bytes without reserializing StationEvent data`
- L135: `rejects Secp256r1 offsets that do not reference the exact serialized StationEvent`
- L149: `rejects measured transaction size above 1232 bytes`
- L165: `requires connected wallet to equal the transition authority encoded by current state`
- L185: `requires connected wallet to support the requested transaction version`

## `node_modules/@lastro/web/tests/solana/wallet.test.ts`

- L35: `connects through Wallet Standard discovery using the configured Solana client integration`
- L53: `reports missing wallet explicitly and never fabricates a connected authority`
- L65: `never requests stores or logs wallet private key material`
- L96: `enumerates public wallet choices and connects one explicitly by name`
- L116: `never falls back to another wallet when an explicit wallet name is missing`

## `node_modules/@lastro/web/tests/verify/chain.test.ts`

- L341: `checks canonical accounts but does not mark transactionless evidence as final valid proof`
- L353: `rejects a package whose terminal custodian differs from canonical RPC state`
- L365: `rejects every terminal RFID revision sequence or last-event-hash mismatch`
- L381: `represents RPC unavailability as NOT_CHECKED and never as canonical validity`
- L397: `requires the AnimalState account owner and binary layout to match the Lastro program`
- L425: `rejects a mixed set of present and missing transaction signatures`
- L444: `binds every txSignature to the exact finalized Lastro transaction envelope`
- L461: `rejects non-finalized and failed evidence transactions`
- L481: `rejects a finalized transaction whose signature differs from EvidencePackage`
- L500: `rejects tampered StationEvent bytes program or accounts in finalized transactions`
- L536: `times out stalled RPC as NOT_CHECKED`

## `node_modules/@lastro/web/tests/verify/package.test.ts`

- L23: `valid frozen package passes every local verification layer before RPC`
- L36: `one changed signed event byte makes the package invalid`
- L47: `changed observed RFID value breaks the event-to-physical-evidence binding`
- L60: `omitting a middle event is detected by sequence and predecessor validation`
- L73: `reordering individually valid signed events invalidates the history`
- L86: `detects a fork even when each individual Station signature is valid`
- L99: `rejects later use of the retired RFID after REIDENTIFY`

## `services/agent/tests/command_contract.rs`

- L48: `command_has_no_new_rfid_hash_field`
- L66: `origin_command_uses_zero_old_rfid_and_predecessor`
- L81: `transfer_command_uses_current_rfid_and_revision`
- L96: `reidentify_command_carries_old_binding_only`

## `services/agent/tests/config.rs`

- L37: `agent_requires_every_transport_identity_database_and_timing_value`
- L62: `agent_rejects_invalid_transport_identity_database_and_timing_values`
- L98: `agent_rejects_short_token_without_leaking_it`
- L113: `agent_valid_configuration_preserves_serial_and_retry_values_exactly`

## `services/agent/tests/retry.rs`

- L143: `api_timeout_keeps_outbox_local`
- L172: `api_transport_errors_do_not_persist_url_credentials`
- L221: `api_500_retries_with_bounded_backoff`
- L258: `restart_resumes_local_rows`
- L286: `server_ack_moves_local_to_server`
- L319: `finalization_moves_server_to_finalized`

## `services/agent/tests/serial_codec.rs`

- L21: `decoder_accepts_frame_split_at_every_byte`
- L41: `decoder_rejects_bad_crc`
- L57: `decoder_rejects_oversized_payload`
- L70: `decoder_resynchronizes_after_noise`
- L86: `encoder_matches_firmware_command_vector`
- L102: `event_ready_decoder_preserves_event_signature_and_rfid`
- L116: `arbitrary_serial_bytes_never_panic`

## `services/agent/tests/serial_payload.rs`

- L18: `command_fixture_is_exactly_224_bytes_and_roundtrips_every_field`
- L39: `command_reserved_bytes_and_unknown_action_are_rejected`
- L57: `event_ready_fixture_is_exactly_397_bytes_and_preserves_signed_event`
- L74: `ack_fixture_binds_capture_id_to_exact_event_hash`
- L87: `error_payload_accepts_only_defined_codes_and_zero_reserved_bytes`

## `services/agent/tests/spool.rs`

- L36: `identical_duplicate_is_idempotent`
- L53: `divergent_duplicate_is_conflict`
- L70: `pending_rows_are_ordered_deterministically`
- L94: `finalized_row_is_immutable`
- L117: `sqlite_rejects_non_eight_byte_observed_rfid`

## `services/agent/tests/worker.rs`

- L159: `worker_sends_one_command_per_station_capture`
- L183: `worker_does_not_ack_before_sqlite_persist`
- L204: `worker_rejects_capture_id_mismatch`
- L232: `worker_rejects_station_pubkey_change`
- L255: `worker_never_modifies_event_bytes`
- L285: `worker_replays_ack_for_durable_local_evidence_after_restart`
- L330: `worker_refuses_to_ack_durable_evidence_from_an_unexpected_station_key`

## `services/api/tests/agent_evidence.rs`

- L139: `accepts_evidence_only_when_event_matches_capture_context`
- L179: `rejects_event_with_wrong_animal_id`
- L207: `rejects_wrong_sequence_revision_or_predecessor`
- L240: `rejects_station_id_not_derived_from_submitted_pubkey`
- L270: `rejects_pubkey_not_registered_for_deployment`
- L302: `rejects_rfid_hash_not_matching_observed_rfid`
- L327: `rejects_invalid_p256_signature`
- L354: `identical_duplicate_is_idempotent`
- L402: `divergent_duplicate_is_conflict`

## `services/api/tests/animals.rs`

- L78: `create_animal_generates_random_32_byte_id`
- L104: `create_animal_does_not_create_onchain_state`
- L129: `duplicate_visual_recovery_id_returns_conflict`
- L148: `get_by_current_rfid_returns_only_current_binding`
- L200: `get_unknown_animal_returns_not_found`
- L225: `retired_rfid_hash_does_not_resolve_as_current_after_reidentify`
- L282: `lookup_by_rfid_never_returns_two_animals`

## `services/api/tests/authorization_boundaries.rs`

- L128: `agent_endpoints_require_the_exact_bearer_token`
- L151: `public_evidence_package_endpoint_does_not_require_agent_authentication`
- L170: `wallet_private_key_fields_are_rejected_by_request_schemas`

## `services/api/tests/captures.rs`

- L140: `origin_capture_derives_sequence_revision_and_zero_predecessor`
- L166: `transfer_capture_derives_context_from_canonical_projection`
- L190: `reidentify_capture_has_no_new_rfid_from_backend`
- L220: `action_specific_next_custodian_rules_are_enforced`
- L238: `capture_creation_does_not_claim_wallet_authorization`
- L307: `second_active_capture_same_station_conflicts`
- L326: `expired_capture_cannot_accept_evidence`
- L390: `station_id_fixture_matches_capture_configuration`
- L406: `dispatched_capture_is_redelivered_identically_until_evidence_is_accepted`
- L447: `accepted_unfinalized_event_reuses_identical_capture_and_blocks_different_intent`

## `services/api/tests/concurrency.rs`

- L103: `concurrent_create_capture_same_station_has_single_winner`
- L145: `concurrent_identical_evidence_is_idempotent`
- L177: `concurrent_divergent_same_sequence_has_single_immutable_result`

## `services/api/tests/config.rs`

- L43: `startup_rejects_every_missing_required_variable_individually`
- L67: `startup_rejects_invalid_fixed_length_crypto_identifiers`
- L98: `startup_rejects_invalid_network_and_database_endpoints`
- L121: `startup_rejects_short_demo_agent_token_without_leaking_it`
- L136: `startup_accepts_one_complete_valid_environment_without_rewriting_values`

## `services/api/tests/confirm.rs`

- L188: `submit_requires_exact_transaction_at_confirmed_commitment`
- L206: `submit_persists_verified_signature_without_advancing_projection`
- L223: `submitted_transaction_is_idempotent_recoverable_and_cannot_be_reprepared`
- L259: `confirm_rejects_transaction_not_preverified_at_confirmed_commitment`
- L275: `confirm_requires_transaction_to_exist_on_finalized_rpc`
- L297: `confirm_reads_animal_state_and_rfid_binding_from_rpc`
- L334: `confirm_rejects_rpc_state_not_matching_event_terminal`
- L355: `confirm_is_idempotent_for_same_finalized_transaction`
- L396: `confirm_does_not_accept_wrong_program_id`

## `services/api/tests/crypto_unit.rs`

- L25: `valid_origin_evidence_preserves_exact_station_bytes`
- L36: `tampered_signature_or_observed_rfid_is_rejected`
- L50: `unregistered_station_key_is_rejected`

## `services/api/tests/database.rs`

- L121: `migration_applies_on_empty_postgres`
- L183: `visual_recovery_id_is_unique`
- L199: `current_rfid_hash_is_unique_when_present`
- L229: `event_bytes_requires_exactly_276_bytes`
- L260: `event_pubkey_requires_33_bytes`
- L291: `event_signature_requires_64_bytes`
- L322: `animal_sequence_is_unique`
- L350: `only_one_active_capture_per_station`
- L381: `capture_context_is_immutable_and_lifecycle_is_monotonic`
- L439: `event_evidence_columns_are_immutable`
- L485: `event_status_and_tx_signature_may_advance_without_mutating_evidence`
- L521: `event_lifecycle_is_monotonic_and_transaction_signature_is_stable`
- L578: `submitted_and_finalized_rows_require_a_transaction_signature`
- L607: `events_and_captures_require_existing_animal_row`
- L653: `finalized_transaction_signature_cannot_be_attached_to_two_events`

## `services/api/tests/errors.rs`

- L18: `validation_errors_are_400_with_stable_public_shape`
- L31: `state_conflicts_are_409_with_stable_public_shape`
- L44: `unavailable_dependencies_are_503_without_internal_details`
- L57: `internal_and_configuration_errors_do_not_leak_secrets`

## `services/api/tests/evidence.rs`

- L126: `evidence_package_orders_events_by_sequence`
- L148: `evidence_package_contains_original_event_bytes`
- L169: `evidence_package_contains_observed_rfid_pubkey_signature_tx`
- L192: `evidence_package_excludes_backend_valid_boolean`
- L214: `evidence_package_matches_schema_contract_and_strict_serde`

## `services/api/tests/health.rs`

- L24: `health_ok_requires_database_and_rpc`
- L40: `health_degraded_when_rpc_unavailable`
- L56: `health_degraded_when_database_unavailable`

## `services/api/tests/resource_limits.rs`

- L111: `oversized_body_is_rejected_before_json_or_dependencies`
- L128: `oversized_base64_evidence_is_rejected_by_the_body_limit`
- L154: `field_bounds_and_malformed_encodings_fail_before_dependencies`
- L197: `malformed_json_wrong_content_type_and_unexpected_fields_are_rejected`
- L235: `repeated_oversized_requests_remain_bounded`

## `services/api/tests/transaction_builder.rs`

- L72: `origin_transaction_places_secp_before_lastro`
- L84: `transfer_transaction_places_secp_before_lastro`
- L99: `reidentify_transaction_places_secp_before_lastro`
- L116: `secp_message_offset_is_derived_from_serialized_lastro_instruction`
- L130: `secp_message_length_is_276`
- L141: `secp_uses_submitted_station_pubkey_and_signature`
- L153: `transaction_uses_current_custodian_as_required_signer`
- L175: `serialized_transaction_size_is_measured_for_every_action`
- L187: `secp_message_is_raw_station_event_not_event_hash`
- L203: `secp_instruction_data_uses_official_u16_offset_layout_and_is_113_bytes`
- L233: `action_account_sets_are_minimal_and_explicit`

## `tests/repository/test_config_syntax.py`

- L33: `test_every_json_file_parses_with_standard_json_parser`
- L41: `test_every_toml_file_parses_with_python_tomllib`
- L49: `test_every_yaml_file_parses_with_pyyaml`
- L57: `test_every_shell_script_passes_bash_syntax_check`
- L64: `test_every_python_file_compiles_without_writing_bytecode_into_repo`
- L70: `test_github_workflow_python_heredocs_compile`
- L93: `test_initialize_protocol_config_cli_loads_from_repository_root`

## `tests/repository/test_contract_schemas.py`

- L12: `test_evidence_package_schema_is_valid_draft_2020_12_and_accepts_both_structural_fixtures`
- L23: `test_evidence_schema_freezes_protocol_lengths_in_transport_encoding`
- L37: `test_openapi_contains_exact_hackathon_surface_and_no_compliance_endpoint`
- L60: `test_openapi_transaction_contract_freezes_two_instructions_and_1232_byte_limit`
- L71: `test_openapi_submission_lifecycle_is_explicit_and_does_not_advance_projection`
- L84: `test_openapi_agent_command_cannot_supply_new_rfid_hash`
- L93: `test_openapi_freezes_http_and_transaction_identifier_resource_bounds`

## `tests/repository/test_demo_preflight_contract.py`

- L6: `test_demo_preflight_is_fail_closed_and_does_not_claim_external_gates`

## `tests/repository/test_firmware_security_contract.py`

- L8: `test_station_signer_never_programs_or_changes_efuse_state`
- L24: `test_efuse_signer_is_explicit_and_fail_closed`
- L47: `test_board_entrypoint_wires_agent_transport_while_reader_protocol_stays_isolated`
- L75: `test_firmware_ci_compiles_efuse_signer_without_provisioning_commands`

## `tests/repository/test_local_dev_environment.py`

- L18: `test_local_env_separates_container_rpc_from_browser_and_system_rpc`
- L29: `test_temporary_program_identity_restores_tracked_source_on_success_and_failure`
- L50: `test_validator_command_uses_real_test_validator_with_explicit_local_dev_bind`
- L62: `test_local_compose_override_only_bridges_api_to_the_host_validator`
- L69: `test_local_runtime_state_is_gitignored_and_core_source_has_no_local_dev_dependency`
- L87: `test_compose_environment_overrides_hostile_shell_deployment_values`
- L120: `test_local_program_build_restores_anchor_files_and_removes_temporary_deploy_key`

## `tests/repository/test_manifest.py`

- L32: `test_manifest_covers_every_tracked_file_once_and_hashes_match`
- L47: `test_manifest_generator_ignores_untracked_local_files`

## `tests/repository/test_protocol_constants.py`

- L4: `test_station_event_length_is_consistent_across_languages_and_docs`

## `tests/repository/test_secret_hygiene.py`

- L42: `test_no_high_confidence_secret_material_is_tracked`
- L64: `test_public_vite_configuration_names_cannot_be_secret_bearing`
- L79: `test_private_runtime_material_is_gitignored`
- L90: `test_no_high_confidence_secret_material_exists_in_reachable_history`

## `tests/repository/test_structure.py`

- L7: `test_no_empty_files`
- L15: `test_no_nested_readmes_or_source_markdown`
- L28: `test_all_production_layers_exist`
- L36: `test_root_cargo_workspace_does_not_embed_anchor_nested_workspace`
- L44: `test_static_vite_container_requires_all_public_build_arguments`
- L59: `test_chain_toolchain_is_isolated_and_pinned`
- L66: `test_litesvm_chain_tests_enable_native_precompiles`
- L73: `test_anchor_litesvm_workspace_declares_rust_test_script_and_skips_validator`
- L80: `test_program_identity_bootstrap_keeps_real_and_ci_modes_separate`
- L90: `test_full_stack_playwright_is_serial_and_has_no_retry_masking`
- L96: `test_github_actions_use_fixed_ubuntu_runner_label`
- L104: `test_ci_uses_committed_resolver_lockfiles_without_fallback_resolution`
- L119: `test_dependency_bootstrap_refreshes_manifest_after_resolver_outputs`
- L127: `test_github_actions_are_pinned_to_immutable_commits`
- L138: `test_container_builds_use_committed_dependency_locks`
- L147: `test_devnet_smoke_requires_committed_program_identity`
- L153: `test_pinned_rust_action_selects_exact_toolchain_explicitly`
- L160: `test_python_ci_uses_exact_patch_without_mutable_pip_upgrade`
- L167: `test_node_and_npm_resolvers_are_exactly_pinned`
- L179: `test_devnet_artifacts_do_not_persist_secret_rpc_or_key_material`
- L197: `test_local_rust_commands_use_locked_dependency_graphs`
- L211: `test_web_ci_uses_only_locked_local_npm_executables`
- L225: `test_production_container_bases_are_immutable_and_runtime_has_no_package_resolution`
- L241: `test_ci_runs_pure_system_harness_tests_without_external_gate_flags`
- L249: `test_solana_cli_checks_require_exact_4_1_2_patch`

## `tests/repository/test_test_contract_quality.py`

- L15: `test_rust_contracts_define_purpose_assertion_and_failure_semantics`
- L41: `test_unity_contracts_define_purpose_assertion_and_failure_semantics`
- L58: `test_browser_contracts_define_arrange_action_assert_and_failure_semantics`
- L78: `test_hardware_and_system_pytest_contracts_define_full_execution_semantics`

## `tests/system/test_demo_stability.py`

- L23: `test_g5_complete_demo_runs_three_times_without_manual_state_repair`

## `tests/system/test_devnet.py`

- L30: `test_g4_complete_flow_on_devnet_and_records_evidence_references`
- L63: `test_g4_stale_custodian_attack_is_rejected_on_devnet_without_state_change`

## `tests/system/test_e2e_controller.py`

- L61: `test_evidence_response_fault_happens_only_after_upstream_acceptance_and_only_once`
- L125: `test_observation_control_endpoint_maps_only_the_three_protocol_actions`

## `tests/system/test_full_local.py`

- L15: `test_full_local_origin_transfer_reidentify_transfer`
- L43: `test_full_local_database_never_advances_before_rpc_confirmation`

## `tests/system/test_recovery.py`

- L20: `test_reidentify_recovers_by_visual_id_without_creating_new_animal`
- L37: `test_known_current_rfid_hash_resolves_exactly_one_active_animal`
- L61: `test_retired_rfid_does_not_resolve_as_current_after_reidentify`
- L86: `test_both_physical_identifiers_missing_remains_unresolved_in_hackathon_scope`

## `tests/system/test_support.py`

- L36: `test_system_crc32c_and_serial_frame_contract_round_trip`
- L52: `test_system_command_decoder_matches_frozen_offsets`
- L79: `test_system_p256_signer_emits_compact_low_s_signature`
- L96: `test_system_pda_derivation_uses_canonical_off_curve_bump`
- L125: `test_system_legacy_message_compiler_preserves_instruction_account_order`
- L154: `test_system_http_error_message_is_strict_json_message_only`
- L168: `test_system_verifier_binds_finalized_signature_to_exact_event_envelope`

## `tests/system/test_tamper.py`

- L13: `test_exported_real_package_fails_after_exactly_one_signed_byte_changes`
- L33: `test_observed_rfid_tamper_is_detected_even_when_signed_event_bytes_are_unchanged`
