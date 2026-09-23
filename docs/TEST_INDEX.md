# Complete Test Index

This file is generated from the repository's real test declarations. Test files are the source of truth; this index supports coverage review and navigation.

**Declared cases:** 544
**Files containing test cases:** 95

Ignored or environment-gated cases are not proven by declaration alone; only an executed non-skipped run counts as validation evidence.

## `apps/web/e2e/demo.spec.ts`

- L19: `full ORIGIN -> A->B -> REIDENTIFY -> B->C flow preserves one AnimalID`
- L117: `stale Wallet A attack after A->B is rejected and cannot change canonical state`

## `apps/web/e2e/failure_recovery.spec.ts`

- L19: `lost API response followed by Agent retry does not create duplicate evidence or transitions`
- L50: `page reload reconstructs UI from durable projection/evidence rather than browser memory`
- L98: `wallet rejection leaves accepted physical evidence non-finalized and canonical state unchanged`
- L139: `reload after wallet broadcast but before SUBMITTED registration reuses the same transaction signature`
- L190: `reload after durable SUBMITTED state resumes finalization without a second wallet signature`

## `apps/web/e2e/layout-16x9.spec.ts`

- L69: `landing keeps all three primary paths inside the first 1600x900 viewport`

## `apps/web/e2e/reidentify.spec.ts`

- L19: `visual recovery resolves the existing AnimalID before a new RFID capture`
- L63: `retired RFID A is no longer presented or resolved as current after REIDENTIFY to B`
- L97: `missing both RFID and visual recovery identifier leaves identity UNRESOLVED`

## `apps/web/e2e/storytelling.spec.ts`

- L10: `landing, problem and future routes render the intended product narrative`
- L53: `presentation pages do not overflow horizontally on the mobile target viewport`

## `apps/web/e2e/verifier.spec.ts`

- L18: `exported package from a real completed flow verifies all five layers`
- L52: `one-byte mutation of signed event data makes the verifier explicitly INVALID`
- L88: `local-file verification continues when the Lastro evidence API is unavailable`

## `apps/web/tests/api/client.test.ts`

- L38: `createAnimal sends only the visual recovery identifier required by hackathon scope`
- L52: `maps every non-success API response to an explicit typed client error`
- L68: `preserves evidence package payload exactly for the verifier layer`
- L79: `network timeout or abort never produces an optimistic success result`
- L107: `requests capture authorization before sending the signed one-time proof`
- L165: `parses submitted transaction metadata separately from capture status`
- L186: `submits only the transaction signature to the confirmed-verification endpoint`
- L210: `rejects inconsistent capture event lifecycle metadata`
- L236: `rejects non-submitted statuses from the transaction submission endpoint`
- L255: `rejects malformed or oversized durable identifiers`

## `apps/web/tests/components/AnimalState.test.ts`

- L22: `renders every canonical/projection field required by the demo state contract`
- L43: `renders pre-ORIGIN absence explicitly without inventing canonical values`

## `apps/web/tests/components/CustodyTimeline.test.ts`

- L12: `preserves every event including REIDENTIFY in canonical sequence order`
- L35: `does not collapse distinct events that have duplicate-looking labels`

## `apps/web/tests/components/StationPanel.test.ts`

- L12: `renders only hardware facts actually represented by the hackathon Station contract`
- L29: `renders unknown Station facts explicitly rather than defaulting to ready`

## `apps/web/tests/components/VerificationPanel.test.ts`

- L21: `renders all five verification layers independently`
- L34: `keeps the invalid layer and concrete failure reason visible`

## `apps/web/tests/config.test.ts`

- L32: `rejects every missing required VITE deployment variable independently`
- L42: `fails closed for canonical verification when no independent authority anchor is supplied`
- L54: `rejects malformed or unsupported API and RPC URL schemes`
- L79: `requires an explicit Wallet Standard solana chain identifier`
- L97: `requires a canonical lowercase 32-byte deployment id`
- L112: `normalizes only API trailing slash and preserves deployment identity values`
- L129: `contains only public deployment values and never exposes server or private-key secrets`

## `apps/web/tests/demo/pendingOperation.test.ts`

- L37: `round-trips the pending operation exactly and clears it explicitly`
- L51: `fails closed and removes malformed local storage`
- L66: `rejects malformed or oversized durable identifiers from local storage`

## `apps/web/tests/pages/DemoPage.test.ts`

- L182: `enables ORIGIN only for an unoriginated animal with a connected intended custodian wallet`
- L203: `normal TRANSFER action is available only to the current custodian wallet`
- L221: `REIDENTIFY uses visual ID for recovery but still requires current custodian authority`
- L240: `exposes the stale-custodian attack only in the explicit stale-authority demo state`
- L264: `authorizes the exact capture intent before reserving Station work`
- L322: `wallet signature rejection leaves canonical and projected state unchanged`
- L359: `persists signed transaction bytes before an ambiguous RPC send failure`
- L409: `does not display fabricated canonical success when API projection is unavailable`
- L428: `restores selected animal and timeline from durable sources after page reload`
- L451: `connects the explicitly selected Wallet Standard wallet`
- L475: `reports physical identity continuity as unresolved when no identifier resolves an animal`
- L491: `reload resumes a durable SUBMITTED transaction without signing again`
- L545: `reload rebroadcasts the exact remembered signed transaction when confirmation is initially absent`
- L610: `retains physical evidence while clearing only a provably expired signed envelope`
- L671: `requests an explicitly signed same-action RFID rescan`
- L733: `reload re-verifies a locally remembered broadcast signature without signing a second transaction`
- L785: `recovers a pending transfer before requesting the finalized evidence timeline`

## `apps/web/tests/pages/StoryRoutes.test.ts`

- L19: `exposes the intended public information architecture`
- L32: `renders the landing paths with Demo as the primary action`
- L50: `renders the sourced physical trust gap story`
- L69: `separates proven functionality from the future expansion path`

## `apps/web/tests/pages/VerifyPage.test.ts`

- L73: `accepts a local evidence package and runs verification without the Lastro evidence API`
- L99: `initial verification layers are NOT_CHECKED rather than optimistic VALID`
- L115: `malformed or unsupported packages can never reach a canonical VALID result`
- L148: `shows RPC unavailability separately from local evidence validity`

## `apps/web/tests/protocol/evidence.test.ts`

- L12: `rejects histories above the bounded verification budget before allocating decoded events`
- L23: `accepts the strict frozen EvidencePackage fixture without rewriting proof bytes`
- L35: `rejects unsupported evidence package versions`
- L47: `rejects malformed encodings missing fields wrong lengths and undeclared verdict fields`
- L71: `round trips evidence transport without changing event order or signed bytes`
- L81: `rejects malformed transaction signatures`

## `apps/web/tests/protocol/hash.test.ts`

- L14: `rfid hash matches frozen cross-language vectors for both physical tags`
- L25: `station id matches the frozen compressed-key vector`
- L37: `event hash matches every frozen 276-byte StationEvent vector`
- L51: `does not hash reader text or framing as if it were canonical RFID bytes`

## `apps/web/tests/protocol/p256.test.ts`

- L31: `verifies every frozen low-S P-256 fixture over its exact raw StationEvent`
- L50: `rejects a signature when one StationEvent byte changes`
- L68: `rejects an otherwise valid signature under a different Station public key`
- L86: `rejects malformed compressed-key and compact-signature encodings`
- L103: `rejects the high-S equivalent of a valid signature`
- L122: `requires raw 276-byte event input and cannot verify by passing event_hash as message`
- L139: `maps malformed off-curve public keys to false instead of throwing`

## `apps/web/tests/protocol/stationEvent.test.ts`

- L13: `decodes the complete 276-byte ORIGIN fixture field by field`
- L31: `decodes sequence and revision with the protocol little-endian convention`
- L47: `rejects wrong magic version action and non-zero reserved bytes`
- L66: `rejects any StationEvent length other than exactly 276 bytes`
- L78: `rejects action-specific semantic combinations that cannot represent a valid transition`

## `apps/web/tests/solana/transaction.test.ts`

- L121: `rejects transaction data targeting a program id different from configured Lastro`
- L134: `rejects an internally valid transaction when it differs from the independently expected intent`
- L160: `requires Secp256r1 verification immediately before the bound Lastro instruction`
- L175: `preserves validated raw instruction bytes without reserializing StationEvent data`
- L189: `rejects Secp256r1 offsets that do not reference the exact serialized StationEvent`
- L205: `rejects measured transaction size above 1232 bytes`
- L221: `requires connected wallet to equal the transition authority encoded by current state`
- L245: `requires connected wallet to support the requested transaction version`

## `apps/web/tests/solana/wallet.test.ts`

- L54: `connects through Wallet Standard discovery using the configured Solana client integration`
- L72: `reports missing wallet explicitly and never fabricates a connected authority`
- L86: `never requests stores or logs wallet private key material`
- L119: `enumerates public wallet choices and connects one explicitly by name`
- L141: `signs only the exact trusted capture-authorization message`
- L198: `binds an explicit RFID rescan to one accepted capture ID`
- L243: `never falls back to another wallet when an explicit wallet name is missing`

## `apps/web/tests/verify/chain.test.ts`

- L380: `refuses an unpinned deployment authority and rejects an authority substituted on-chain`
- L397: `checks canonical accounts but does not mark transactionless evidence as final valid proof`
- L409: `rejects a package whose terminal custodian differs from canonical RPC state`
- L423: `rejects every terminal RFID revision sequence or last-event-hash mismatch`
- L447: `represents RPC unavailability as NOT_CHECKED and never as canonical validity`
- L469: `requires the AnimalState account owner and binary layout to match the Lastro program`
- L517: `rejects a mixed set of present and missing transaction signatures`
- L537: `binds every txSignature to the exact finalized Lastro transaction envelope`
- L555: `rejects non-finalized and failed evidence transactions`
- L591: `rejects a finalized transaction whose signature differs from EvidencePackage`
- L611: `rejects tampered StationEvent bytes program or accounts in finalized transactions`
- L648: `times out stalled RPC as NOT_CHECKED`

## `apps/web/tests/verify/package.test.ts`

- L24: `valid frozen package passes every local verification layer before RPC`
- L43: `one changed signed event byte makes the package invalid`
- L54: `changed observed RFID value breaks the event-to-physical-evidence binding`
- L67: `omitting a middle event is detected by sequence and predecessor validation`
- L80: `reordering individually valid signed events invalidates the history`
- L93: `detects a fork even when each individual Station signature is valid`
- L106: `rejects later use of the retired RFID after REIDENTIFY`

## `chain/programs/lastro/tests/account_constraints.rs`

- L8: `animal_state_pda_uses_deployment_and_animal_id`
- L32: `rfid_binding_pda_uses_deployment_and_rfid_hash`
- L56: `protocol_config_pda_uses_deployment_id`
- L79: `wrong_pda_accounts_are_rejected`

## `chain/programs/lastro/tests/initialize.rs`

- L9: `initialize_creates_config_with_deployment_and_station`
- L39: `initialize_rejects_duplicate_config_pda`
- L61: `initialize_rejects_invalid_station_pubkey_encoding`

## `chain/programs/lastro/tests/origin.rs`

- L11: `origin_creates_animal_state_and_active_rfid_binding`
- L44: `origin_requires_sequence_one_revision_one_zero_predecessor`
- L85: `origin_requires_to_custodian_signer`
- L109: `origin_rejects_existing_animal_state`
- L142: `origin_rejects_rfid_ever_bound_to_another_history`

## `chain/programs/lastro/tests/reidentify.rs`

- L19: `reidentify_preserves_animal_id_and_custodian`
- L46: `reidentify_increments_revision_and_sequence_once`
- L76: `reidentify_retires_old_binding_and_activates_new`
- L109: `reidentify_rejects_same_new_rfid`
- L134: `reidentify_rejects_new_rfid_already_known`
- L206: `reidentify_requires_current_custodian_signer`

## `chain/programs/lastro/tests/rfid_binding.rs`

- L9: `only_one_active_binding_can_point_to_an_rfid`
- L42: `retired_binding_keeps_original_animal_id`
- L59: `retired_rfid_cannot_be_reoriginated`
- L87: `lookup_active_binding_matches_animal_state_current_rfid`

## `chain/programs/lastro/tests/secp_binding.rs`

- L9: `valid_secp_over_exact_event_is_accepted`
- L33: `missing_secp_instruction_fails`
- L55: `wrong_station_pubkey_fails`
- L85: `signature_and_public_key_descriptor_offsets_and_indexes_are_exact`
- L134: `signature_over_other_276_bytes_fails`
- L163: `message_offset_one_byte_wrong_fails`
- L200: `message_length_275_or_277_fails`
- L234: `wrong_instruction_index_fails`
- L266: `secp_after_lastro_fails_when_program_requires_expected_order`
- L289: `high_s_signature_is_rejected_by_runtime_precompile`
- L315: `extra_instruction_fails_frozen_two_instruction_envelope`

## `chain/programs/lastro/tests/state_machine.rs`

- L10: `full_origin_transfer_reidentify_transfer_reaches_expected_terminal_state`
- L40: `replay_of_any_consumed_event_fails`
- L141: `stale_revision_after_reidentify_fails`
- L169: `old_rfid_after_reidentify_fails`
- L202: `fork_with_valid_station_signature_but_wrong_predecessor_fails`

## `chain/programs/lastro/tests/transaction_size.rs`

- L22: `origin_serialized_transaction_fits_selected_format_limit`
- L39: `transfer_serialized_transaction_fits_selected_format_limit`
- L56: `reidentify_serialized_transaction_fits_selected_format_limit`
- L73: `size_test_includes_wallet_signature_accounts_and_both_instructions`

## `chain/programs/lastro/tests/transfer.rs`

- L25: `transfer_a_to_b_updates_only_custodian_sequence_hash`
- L55: `old_custodian_cannot_transfer_after_a_to_b`
- L93: `transfer_requires_physical_rfid_equal_current_binding`
- L114: `transfer_rejects_sequence_gap`
- L138: `transfer_rejects_wrong_predecessor`
- L171: `transfer_rejects_revision_change`
- L197: `transfer_rejects_same_destination_as_current_custodian`

## `crates/lastro-protocol/tests/decode_rejection.rs`

- L8: `rejects_275_bytes`
- L19: `rejects_277_bytes`
- L32: `rejects_wrong_magic`
- L45: `rejects_unknown_version`
- L58: `rejects_unknown_action`
- L71: `rejects_nonzero_reserved`
- L84: `arbitrary_bytes_never_panic_and_non_exact_lengths_never_decode`

## `crates/lastro-protocol/tests/evidence_package.rs`

- L20: `package_preserves_event_order`
- L37: `package_rejects_wrong_event_length`
- L52: `package_rejects_wrong_key_signature_lengths`
- L71: `package_does_not_trust_valid_flag`
- L86: `package_detects_one_byte_evidence_tampering`
- L98: `package_rejects_noncanonical_signature_text`

## `crates/lastro-protocol/tests/ids.rs`

- L8: `animal_id_is_exactly_32_bytes`
- L16: `station_id_matches_domain_separated_pubkey_hash`
- L31: `invalid_p256_pubkey_length_is_rejected`

## `crates/lastro-protocol/tests/layout.rs`

- L11: `station_event_is_exactly_276_bytes`
- L19: `all_offsets_are_contiguous_and_end_at_276`
- L41: `header_is_lstr_version_one_and_reserved_zero`
- L53: `integers_are_little_endian`

## `crates/lastro-protocol/tests/origin.rs`

- L20: `origin_requires_sequence_one`
- L30: `origin_requires_revision_one`
- L40: `origin_requires_zero_predecessor_and_old_rfid`
- L53: `origin_requires_zero_from_and_nonzero_to`

## `crates/lastro-protocol/tests/p256.rs`

- L8: `valid_compact_low_s_signature_verifies_raw_event`
- L28: `one_byte_event_change_breaks_signature`
- L52: `wrong_station_key_breaks_signature`
- L73: `high_s_signature_is_rejected`

## `crates/lastro-protocol/tests/reidentify.rs`

- L8: `reidentify_requires_new_rfid`
- L21: `reidentify_keeps_custodian`
- L34: `reidentify_advances_revision_exactly_once`

## `crates/lastro-protocol/tests/rfid.rs`

- L11: `same_canonical_rfid_hashes_identically`
- L20: `different_canonical_rfid_values_hash_differently`
- L30: `rfid_hash_uses_exact_domain_separator`
- L43: `formatted_display_text_is_not_silently_canonical`
- L54: `canonical_rfid_is_exactly_8_bytes`

## `crates/lastro-protocol/tests/transfer.rs`

- L8: `transfer_keeps_same_rfid`
- L21: `transfer_keeps_revision`
- L37: `transfer_rejects_self_destination`

## `crates/lastro-protocol/tests/vectors.rs`

- L18: `origin_fixture_matches_reference`
- L26: `transfer_fixture_matches_reference`
- L34: `reidentify_fixture_matches_reference`

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
- L63: `station releases an RFID wait after its monotonic deadline`
- L86: `station rejects second command while busy`
- L103: `station new RFID hash comes from observed RFID`
- L122: `station ACK returns to idle only for matching event`
- L147: `station invalid transfer observation returns to safe state`
- L172: `station accepts an identical command replay while waiting for RFID`
- L188: `station replays identical signed evidence when the same command returns before ACK`

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

## `services/agent/tests/command_contract.rs`

- L48: `command_has_no_new_rfid_hash_field`
- L66: `origin_command_uses_zero_old_rfid_and_predecessor`
- L81: `transfer_command_uses_current_rfid_and_revision`
- L96: `reidentify_command_carries_old_binding_only`

## `services/agent/tests/config.rs`

- L41: `agent_requires_every_transport_identity_database_and_timing_value`
- L67: `agent_rejects_invalid_transport_identity_database_and_timing_values`
- L107: `agent_rejects_short_token_without_leaking_it`
- L122: `agent_requires_explicit_positive_station_response_timeout`
- L142: `agent_valid_configuration_preserves_serial_and_retry_values_exactly`

## `services/agent/tests/retry.rs`

- L156: `api_timeout_keeps_outbox_local`
- L194: `api_transport_errors_do_not_persist_url_credentials`
- L240: `api_500_retries_with_bounded_backoff`
- L314: `restart_resumes_local_rows`
- L355: `server_ack_moves_local_to_server`
- L410: `finalization_moves_server_to_finalized`

## `services/agent/tests/serial_codec.rs`

- L24: `decoder_accepts_frame_split_at_every_byte`
- L46: `decoder_rejects_bad_crc`
- L69: `decoder_rejects_oversized_payload`
- L82: `decoder_resynchronizes_after_noise`
- L98: `encoder_matches_firmware_command_vector`
- L117: `event_ready_decoder_preserves_event_signature_and_rfid`
- L134: `arbitrary_serial_bytes_never_panic`

## `services/agent/tests/serial_payload.rs`

- L18: `command_fixture_is_exactly_224_bytes_and_roundtrips_every_field`
- L42: `command_reserved_bytes_and_unknown_action_are_rejected`
- L60: `event_ready_fixture_is_exactly_397_bytes_and_preserves_signed_event`
- L80: `ack_fixture_binds_capture_id_to_exact_event_hash`
- L103: `error_payload_accepts_only_defined_codes_and_zero_reserved_bytes`

## `services/agent/tests/spool.rs`

- L50: `identical_duplicate_is_idempotent`
- L67: `divergent_duplicate_is_conflict`
- L87: `pending_rows_are_ordered_deterministically`
- L123: `finalized_row_is_immutable`
- L167: `sqlite_rejects_non_eight_byte_observed_rfid`

## `services/agent/tests/worker.rs`

- L263: `worker_times_out_a_station_that_never_responds`
- L308: `worker_sends_one_command_per_station_capture`
- L346: `worker_does_not_ack_before_sqlite_persist`
- L375: `worker_rejects_capture_id_mismatch`
- L415: `worker_rejects_station_pubkey_change`
- L454: `worker_never_modifies_event_bytes`
- L493: `worker_replays_ack_for_durable_local_evidence_after_restart`
- L541: `worker_replays_ack_for_server_evidence_after_restart`
- L595: `transient_command_poll_failure_does_not_terminate_worker`
- L637: `terminal_api_rejection_quarantines_durable_evidence_without_stopping_retry_processing`
- L698: `worker_refuses_to_ack_durable_evidence_from_an_unexpected_station_key`

## `services/api/tests/agent_evidence.rs`

- L134: `accepts_evidence_only_when_event_matches_capture_context`
- L174: `rejects_event_with_wrong_animal_id`
- L202: `rejects_wrong_sequence_revision_or_predecessor`
- L235: `rejects_station_id_not_derived_from_submitted_pubkey`
- L265: `rejects_pubkey_not_registered_for_deployment`
- L298: `rejects_rfid_hash_not_matching_observed_rfid`
- L323: `rejects_invalid_p256_signature`
- L350: `identical_duplicate_is_idempotent`
- L398: `divergent_duplicate_is_conflict`

## `services/api/tests/animals.rs`

- L76: `anonymous_registration_stops_at_the_database_backed_minute_budget`
- L103: `create_animal_generates_random_32_byte_id`
- L129: `create_animal_does_not_create_onchain_state`
- L154: `duplicate_visual_recovery_id_returns_conflict`
- L177: `get_by_current_rfid_returns_only_current_binding`
- L241: `get_unknown_animal_returns_not_found`
- L266: `retired_rfid_hash_does_not_resolve_as_current_after_reidentify`
- L338: `lookup_by_rfid_never_returns_two_animals`

## `services/api/tests/authorization_boundaries.rs`

- L128: `agent_endpoints_require_the_exact_bearer_token`
- L151: `public_evidence_package_endpoint_does_not_require_agent_authentication`
- L170: `wallet_private_key_fields_are_rejected_by_request_schemas`

## `services/api/tests/capture_authorization.rs`

- L131: `public_capture_challenges_are_bounded_per_custodian`
- L159: `current_custodian_signature_authorizes_transfer_exactly_once`
- L219: `invalid_wallet_signature_never_reserves_a_capture`
- L258: `challenge_cannot_authorize_a_different_capture_intent`
- L290: `expired_challenge_is_rejected_before_capture_reservation`
- L334: `failed_capture_reservation_does_not_consume_valid_authorization`

## `services/api/tests/captures.rs`

- L237: `origin_capture_derives_sequence_revision_and_zero_predecessor`
- L266: `transfer_capture_derives_context_from_canonical_projection`
- L293: `reidentify_capture_has_no_new_rfid_from_backend`
- L326: `action_specific_next_custodian_rules_are_enforced`
- L362: `capture_authorization_stays_separate_from_onchain_transaction_authority`
- L439: `second_active_capture_same_station_conflicts`
- L470: `expired_capture_cannot_accept_evidence`
- L538: `station_id_fixture_matches_capture_configuration`
- L554: `dispatched_capture_is_redelivered_identically_until_evidence_is_accepted`
- L605: `accepted_unsubmitted_event_can_be_superseded_by_new_authorized_intent`
- L740: `submitted_event_cannot_be_superseded_by_a_different_capture_intent`
- L828: `a_signed_explicit_reidentify_recapture_replaces_the_exact_accepted_evidence`

## `services/api/tests/concurrency.rs`

- L107: `concurrent_create_capture_same_station_has_single_winner`
- L149: `concurrent_identical_evidence_is_idempotent`
- L199: `concurrent_divergent_same_sequence_has_single_immutable_result`

## `services/api/tests/config.rs`

- L43: `startup_rejects_every_missing_required_variable_individually`
- L67: `startup_rejects_invalid_fixed_length_crypto_identifiers`
- L98: `startup_rejects_invalid_network_and_database_endpoints`
- L121: `startup_rejects_short_demo_agent_token_without_leaking_it`
- L136: `startup_accepts_one_complete_valid_environment_without_rewriting_values`

## `services/api/tests/confirm.rs`

- L200: `submit_requires_exact_transaction_at_confirmed_commitment`
- L221: `submit_persists_verified_signature_without_advancing_projection`
- L241: `submitted_transaction_is_idempotent_recoverable_and_cannot_be_reprepared`
- L294: `confirm_rejects_transaction_not_preverified_at_confirmed_commitment`
- L313: `confirm_requires_transaction_to_exist_on_finalized_rpc`
- L338: `confirm_reads_animal_state_and_rfid_binding_from_rpc`
- L378: `confirm_rejects_rpc_state_not_matching_event_terminal`
- L402: `confirm_is_idempotent_for_same_finalized_transaction`
- L452: `confirm_does_not_accept_wrong_program_id`

## `services/api/tests/crypto_unit.rs`

- L25: `valid_origin_evidence_preserves_exact_station_bytes`
- L42: `tampered_signature_or_observed_rfid_is_rejected`
- L59: `unregistered_station_key_is_rejected`

## `services/api/tests/database.rs`

- L121: `migration_applies_on_empty_postgres`
- L186: `visual_recovery_id_is_unique`
- L202: `current_rfid_hash_is_unique_when_present`
- L233: `event_bytes_requires_exactly_276_bytes`
- L270: `event_pubkey_requires_33_bytes`
- L307: `event_signature_requires_64_bytes`
- L344: `animal_sequence_is_unique`
- L374: `only_one_active_capture_per_station`
- L413: `capture_context_is_immutable_and_lifecycle_is_monotonic`
- L488: `event_evidence_columns_are_immutable`
- L543: `event_status_and_tx_signature_may_advance_without_mutating_evidence`
- L584: `event_lifecycle_is_monotonic_and_transaction_signature_is_stable`
- L647: `submitted_and_finalized_rows_require_a_transaction_signature`
- L679: `events_and_captures_require_existing_animal_row`
- L733: `finalized_transaction_signature_cannot_be_attached_to_two_events`

## `services/api/tests/errors.rs`

- L18: `validation_errors_are_400_with_stable_public_shape`
- L34: `state_conflicts_are_409_with_stable_public_shape`
- L48: `unavailable_dependencies_are_503_without_internal_details`
- L62: `internal_and_configuration_errors_do_not_leak_secrets`

## `services/api/tests/evidence.rs`

- L182: `evidence_package_orders_events_by_sequence`
- L206: `evidence_package_ignores_rejected_superseded_attempts`
- L239: `evidence_package_contains_original_event_bytes`
- L262: `evidence_package_contains_observed_rfid_pubkey_signature_tx`
- L293: `evidence_package_excludes_backend_valid_boolean`
- L323: `evidence_package_matches_schema_contract_and_strict_serde`

## `services/api/tests/health.rs`

- L33: `health_ok_requires_database_and_rpc`
- L49: `health_degraded_when_rpc_unavailable`
- L65: `health_degraded_when_database_unavailable`

## `services/api/tests/resource_limits.rs`

- L108: `oversized_body_is_rejected_before_json_or_dependencies`
- L125: `oversized_base64_evidence_is_rejected_by_the_body_limit`
- L151: `field_bounds_and_malformed_encodings_fail_before_dependencies`
- L198: `malformed_json_wrong_content_type_and_unexpected_fields_are_rejected`
- L236: `repeated_oversized_requests_remain_bounded`

## `services/api/tests/transaction_builder.rs`

- L83: `origin_transaction_places_secp_before_lastro`
- L98: `transfer_transaction_places_secp_before_lastro`
- L122: `reidentify_transaction_places_secp_before_lastro`
- L156: `secp_message_offset_is_derived_from_serialized_lastro_instruction`
- L176: `secp_message_length_is_276`
- L187: `secp_uses_submitted_station_pubkey_and_signature`
- L199: `transaction_uses_current_custodian_as_required_signer`
- L227: `serialized_transaction_size_is_measured_for_every_action`
- L239: `secp_message_is_raw_station_event_not_event_hash`
- L258: `secp_instruction_data_uses_official_u16_offset_layout_and_is_113_bytes`
- L294: `action_account_sets_are_minimal_and_explicit`

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
- L61: `test_openapi_transaction_contract_freezes_two_instructions_and_1232_byte_limit`
- L72: `test_openapi_submission_lifecycle_is_explicit_and_does_not_advance_projection`
- L85: `test_openapi_agent_command_cannot_supply_new_rfid_hash`
- L94: `test_openapi_freezes_http_and_transaction_identifier_resource_bounds`

## `tests/repository/test_demo_preflight_contract.py`

- L6: `test_demo_preflight_is_fail_closed_and_does_not_claim_external_gates`

## `tests/repository/test_firmware_security_contract.py`

- L8: `test_station_signer_never_programs_or_changes_efuse_state`
- L24: `test_efuse_signer_is_explicit_and_fail_closed`
- L47: `test_board_entrypoint_wires_agent_transport_while_reader_protocol_stays_isolated`
- L75: `test_firmware_ci_compiles_efuse_signer_without_provisioning_commands`

## `tests/repository/test_initialize_funding.py`

- L21: `test_initializer_waits_for_the_finalized_bank_to_credit_authority`
- L30: `test_initializer_rejects_unfunded_authority_without_sending_a_transaction`

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

- L8: `test_no_empty_files`
- L16: `test_no_nested_readmes_or_source_markdown`
- L29: `test_all_production_layers_exist`
- L37: `test_root_cargo_workspace_does_not_embed_anchor_nested_workspace`
- L45: `test_static_vite_container_requires_all_public_build_arguments`
- L62: `test_chain_toolchain_is_isolated_and_pinned`
- L69: `test_litesvm_chain_tests_enable_native_precompiles`
- L76: `test_anchor_litesvm_workspace_declares_rust_test_script_and_skips_validator`
- L83: `test_program_identity_bootstrap_keeps_real_and_ci_modes_separate`
- L93: `test_full_stack_playwright_is_serial_and_has_no_retry_masking`
- L99: `test_github_actions_use_ubuntu_latest_linux_runner`
- L108: `test_github_actions_cancel_superseded_runs_per_workflow_and_ref`
- L117: `test_self_hosted_jobs_clean_workspace_before_checkout`
- L131: `test_firmware_build_does_not_run_the_checkout_inside_a_root_job_container`
- L139: `test_self_hosted_solana_jobs_use_checksum_pinned_anchor_and_solana_binaries`
- L149: `test_firmware_workflow_activates_esp_idf_environment_before_using_idf_py`
- L158: `test_maintenance_sync_stages_only_existing_repository_paths`
- L167: `test_maintenance_sync_covers_every_push_for_manifest_consistency`
- L173: `test_maintenance_sync_executes_repository_generators_on_every_push`
- L179: `test_chain_resolver_lockfile_is_committed`
- L183: `test_postgres_18_compose_persists_the_real_data_root_and_binds_dev_port_to_loopback`
- L190: `test_browser_deployment_id_is_propagated_across_build_and_local_environments`
- L198: `test_local_development_requires_the_same_solana_patch_as_anchor_ci`
- L205: `test_agent_main_reconnects_after_serial_transport_failures`
- L213: `test_web_ci_program_id_matches_transaction_validation_fixture`
- L220: `test_browser_records_signed_solana_identity_before_rpc_broadcast`
- L230: `test_ci_uses_committed_resolver_lockfiles_without_fallback_resolution`
- L245: `test_dependency_bootstrap_refreshes_manifest_after_resolver_outputs`
- L253: `test_github_actions_are_pinned_to_immutable_commits`
- L264: `test_protocol_initialize_transaction_requests_explicit_compute_budget`
- L284: `test_litesvm_test_harness_boxes_large_failure_metadata`
- L293: `test_chain_tests_use_split_solana_4_2_ids_and_traits_explicitly`
- L307: `test_secp256r1_verifier_has_no_obsolete_descriptor_boundary_constant`
- L319: `test_anchor_program_reexports_nested_generated_account_helpers_at_crate_root`
- L329: `test_chain_sources_follow_anchor_1_2_program_api_contract`
- L381: `test_maintenance_sync_regenerates_both_rust_lockfiles`
- L386: `test_chain_rustsec_exceptions_are_narrow_and_documented`
- L404: `test_container_builds_use_committed_dependency_locks`
- L413: `test_devnet_smoke_requires_committed_program_identity`
- L419: `test_pinned_rust_action_selects_exact_toolchain_explicitly`
- L426: `test_python_ci_uses_exact_patch_without_mutable_pip_upgrade`
- L433: `test_root_node_toolchain_pins_typescript_used_by_hoisted_vue_tsc`
- L442: `test_web_typecheck_keeps_strict_source_checks_but_skips_third_party_declarations`
- L455: `test_vite_config_includes_vitest_types_for_inline_test_configuration`
- L462: `test_vue_tsc_uses_typescript_version_known_to_be_supported`
- L470: `test_maintenance_sync_regenerates_node_lock_before_locked_install`
- L475: `test_node_and_npm_resolvers_are_exactly_pinned`
- L487: `test_devnet_artifacts_do_not_persist_secret_rpc_or_key_material`
- L505: `test_local_rust_commands_use_locked_dependency_graphs`
- L519: `test_web_ci_uses_only_locked_local_npm_executables`
- L533: `test_production_container_bases_are_immutable_and_runtime_has_no_package_resolution`
- L549: `test_ci_runs_pure_system_harness_tests_without_external_gate_flags`
- L557: `test_solana_cli_checks_require_exact_4_2_0_patch`

## `tests/repository/test_test_contract_quality.py`

- L15: `test_rust_contracts_define_purpose_assertion_and_failure_semantics`
- L41: `test_unity_contracts_define_purpose_assertion_and_failure_semantics`
- L58: `test_browser_contracts_define_arrange_action_assert_and_failure_semantics`
- L78: `test_hardware_and_system_pytest_contracts_define_full_execution_semantics`
- L94: `test_generated_test_index_has_no_trailing_whitespace`

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

- L37: `test_system_crc32c_and_serial_frame_contract_round_trip`
- L53: `test_system_command_decoder_matches_frozen_offsets`
- L80: `test_system_p256_signer_emits_compact_low_s_signature`
- L98: `test_system_wallet_authorizes_only_the_exact_capture_challenge`
- L161: `test_system_pda_derivation_uses_canonical_off_curve_bump`
- L190: `test_system_legacy_message_compiler_preserves_instruction_account_order`
- L219: `test_system_http_error_message_is_strict_json_message_only`
- L233: `test_system_verifier_binds_finalized_signature_to_exact_event_envelope`

## `tests/system/test_tamper.py`

- L13: `test_exported_real_package_fails_after_exactly_one_signed_byte_changes`
- L33: `test_observed_rfid_tamper_is_detected_even_when_signed_event_bytes_are_unchanged`
