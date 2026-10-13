# Complete Test Index

This file is generated from the repository's real test declarations. Test files are the source of truth; this index supports coverage review and navigation.

**Declared cases:** 439
**Files containing test cases:** 84

Ignored or environment-gated cases are not proven by declaration alone; only an executed non-skipped run counts as validation evidence.

## `apps/web/e2e/layout-16x9.spec.ts`

- L69: `landing keeps all three primary paths inside the first 1600x900 viewport`

## `apps/web/e2e/storytelling.spec.ts`

- L10: `landing, problem and future routes render the intended product narrative`
- L53: `presentation pages do not overflow horizontally on the mobile target viewport`

## `apps/web/tests/api/client.test.ts`

- L55: `reads canonical assets and resolves active or retired RFIDs`
- L74: `maps every non-success API response to an explicit typed client error`
- L90: `preserves evidence package payload exactly for the verifier layer`
- L104: `network timeout never produces an optimistic success result`
- L123: `requests capture authorization before sending the signed one-time proof`
- L162: `rejects inconsistent capture event lifecycle metadata`
- L184: `submits only the transaction signature to the v2 confirmed-verification endpoint`
- L206: `rejects malformed or oversized durable identifiers`

## `apps/web/tests/auth/loginWallets.test.ts`

- L49: `offers every wallet able to sign a Solana message, whatever its cluster`
- L73: `signs in with the Solana account, never the Ethereum one`

## `apps/web/tests/auth/partyRegistry.test.ts`

- L53: `decodes a party of this deployment registered for the wallet`
- L71: `rejects parties of other deployments, other wallets or forged layouts`
- L92: `keeps only locally verified parties from the RPC answer`
- L107: `maps on-chain roles to workspace profiles`
- L136: `keeps the exporter distinct from commerce and shaped like transport`

## `apps/web/tests/auth/walletLogin.test.ts`

- L30: `binds the login message to the site, wallet, deployment and a fresh nonce`
- L58: `accepts only a signature made by the claimed wallet over the exact message`
- L85: `chooses the workspace from the wallet registration status and role`
- L119: `proves control of the wallet before reading its party`

## `apps/web/tests/components/AnimalState.test.ts`

- L24: `renders every canonical field required by the demo state contract`
- L38: `renders the absence of an RFID explicitly`

## `apps/web/tests/components/CustodyTimeline.test.ts`

- L12: `preserves every event including RFID replacement in canonical order`
- L35: `does not collapse distinct events that have duplicate-looking labels`

## `apps/web/tests/components/StationPanel.test.ts`

- L12: `renders only hardware facts actually represented by the hackathon Station contract`
- L29: `renders unknown Station facts explicitly rather than defaulting to ready`

## `apps/web/tests/components/VerificationPanel.test.ts`

- L21: `renders all five verification layers independently`
- L34: `keeps the invalid layer and concrete failure reason visible`

## `apps/web/tests/config.test.ts`

- L33: `rejects every missing required VITE deployment variable independently`
- L49: `keeps wallets self-paid when no fee sponsor is pinned`
- L55: `fails closed for canonical verification when no independent authority anchor is supplied`
- L67: `rejects malformed or unsupported API and RPC URL schemes`
- L92: `requires an explicit Wallet Standard solana chain identifier`
- L110: `requires a canonical lowercase 32-byte deployment id`
- L125: `normalizes only API trailing slash and preserves deployment identity values`
- L143: `contains only public deployment values and never exposes server or private-key secrets`

## `apps/web/tests/demo/pendingOperation.test.ts`

- L35: `round-trips the pending operation exactly and clears it explicitly`
- L49: `fails closed and removes malformed local storage`
- L64: `rejects malformed or oversized durable identifiers from local storage`

## `apps/web/tests/demo/workspace.test.ts`

- L32: `gives each chain participant its own sections and the common user read access`
- L62: `creates, updates and deletes records and keeps them across a reload`
- L90: `fails closed to the example rows when stored records are malformed`
- L105: `restores the example data`
- L121: `remembers wallet and visitor sessions across a reload`
- L147: `drops stored sessions that no login could have produced`

## `apps/web/tests/i18n.test.ts`

- L25: `switches the page between English and Portuguese`
- L51: `remembers the chosen language in this browser`

## `apps/web/tests/pages/ChainHistoryPage.test.ts`

- L23: `renders the product journey newest stage first with the current stage marked`
- L67: `links every stage to its detail page`

## `apps/web/tests/pages/ChainStagePages.test.ts`

- L140: `connects the records back to the animal and its property`
- L165: `derives stable demo identifiers`

## `apps/web/tests/pages/DemoPage.test.ts`

- L161: `offers BIND only to the custodian of an untagged asset`
- L184: `offers REPLACE and presence proof once the asset is tagged`
- L200: `authorizes the exact capture intent before reserving Station work`
- L233: `wallet signature rejection leaves canonical state unchanged`
- L257: `runs a BIND capture to finalized canonical state`
- L309: `reload resumes a durable SUBMITTED transaction without signing again`
- L349: `registers an animal and waits for its finalized AssetState`
- L378: `resolves a retired RFID to its animal`
- L400: `runs a two-phase custody transfer`
- L445: `lets a recipient on another device accept a shared transfer ID`
- L472: `exposes the stale-custodian check without creating work`

## `apps/web/tests/pages/LoginPage.test.ts`

- L74: `signs a registered wallet into the role recorded on Solana`
- L106: `refuses revoked registrations and cancelled signatures`
- L132: `signs in with MetaMask like any other Solana wallet`
- L160: `offers a read-only visitor session when no wallet is installed`

## `apps/web/tests/pages/StoryRoutes.test.ts`

- L19: `exposes the intended public information architecture`
- L52: `renders the landing paths with Demo as the primary action`
- L70: `renders the sourced physical trust gap story`
- L89: `separates proven functionality from the future expansion path`

## `apps/web/tests/pages/VerifyPage.test.ts`

- L64: `accepts a local evidence package and runs verification without the Lastro evidence API`
- L90: `initial verification layers are NOT_CHECKED rather than optimistic VALID`
- L106: `malformed or unsupported packages can never reach a canonical VALID result`
- L139: `shows RPC unavailability separately from local evidence validity`

## `apps/web/tests/pages/WorkspacePage.test.ts`

- L63: `lets the producer create, edit and delete its records`
- L107: `shows the whole chain to the common user without any change action`
- L136: `drops a stored wallet session whose party was revoked on Solana`
- L153: `follows the on-chain role over the stored one`
- L175: `returns to the login after signing out`

## `apps/web/tests/protocol/evidence.test.ts`

- L14: `accepts the strict frozen EvidencePackage fixture without rewriting proof bytes`
- L24: `rejects histories above the bounded verification budget`
- L36: `rejects unsupported evidence package schemas`
- L48: `rejects malformed encodings missing fields undeclared fields and non-final events`

## `apps/web/tests/protocol/hash.test.ts`

- L19: `rfid hash matches frozen cross-language vectors for both physical tags`
- L31: `station id matches the frozen compressed-key vector`
- L43: `event and identifier payload hashes match every v2 capture vector`
- L62: `does not hash reader text or framing as if it were canonical RFID bytes`

## `apps/web/tests/protocol/p256.test.ts`

- L31: `verifies every frozen low-S P-256 fixture over its exact raw envelope`
- L50: `rejects a signature when one envelope byte changes`
- L68: `rejects an otherwise valid signature under a different Station public key`
- L86: `rejects malformed compressed-key and compact-signature encodings`
- L103: `rejects the high-S equivalent of a valid signature`
- L122: `requires raw 220-byte envelope input and cannot verify by passing event_hash as message`
- L139: `maps malformed off-curve public keys to false instead of throwing`

## `apps/web/tests/protocol/v2/envelope.test.ts`

- L32: `uses the fixed 220-byte little-endian layout`
- L47: `round-trips without changing any canonical field`
- L57: `rejects malformed length and zero identifiers`

## `apps/web/tests/protocol/v2/transformation.test.ts`

- L53: `matches the checked-in roots and fixed manifest length`
- L89: `rejects duplicate positions and mass outside tolerance`

## `apps/web/tests/solana/transaction.test.ts`

- L137: `accepts the exact Station-event transaction the API builds`
- L148: `rejects transaction data targeting a program id different from configured Lastro`
- L160: `rejects an internally valid transaction that differs from the expected capture`
- L178: `binds the RFID argument, Station signature and precompile offsets to the envelope`
- L196: `derives every account independently`
- L213: `validates wallet-only registration and custody acceptance instructions`
- L320: `accepts only the pinned sponsor as a separate fee payer`
- L347: `validates session opening and batch anchoring against the displayed terms`
- L501: `requires connected wallet to equal the transition authority`
- L528: `bounds the priority fee a wallet is asked to sign`

## `apps/web/tests/solana/wallet.test.ts`

- L54: `connects through Wallet Standard discovery using the configured Solana client integration`
- L72: `reports missing wallet explicitly and never fabricates a connected authority`
- L86: `never requests stores or logs wallet private key material`
- L119: `enumerates public wallet choices and connects one explicitly by name`
- L140: `signs only the exact trusted capture-authorization message`
- L191: `never falls back to another wallet when an explicit wallet name is missing`

## `apps/web/tests/system/flow.test.ts`

- L21: `runs a normal capture from tag read to a valid independent verification`
- L35: `stops a capture whose bytes were changed in transit at the API`
- L53: `rejects evidence from an unregistered Station`
- L65: `flags a package edited after anchoring at the independent verifier`
- L77: `keeps the event queued while offline and still finishes valid`

## `apps/web/tests/verify/chain.test.ts`

- L259: `accepts a package that matches finalized accounts and exact transactions`
- L270: `refuses an unpinned deployment authority and rejects an authority substituted on-chain`
- L288: `rejects a package whose terminal state differs from canonical AssetState`
- L304: `requires the replaced RFID binding to be RETIRED`
- L329: `binds every txSignature to the exact finalized Lastro transaction`
- L351: `accepts presence proofs anchored by the custodian`
- L369: `accepts history whose fee and rent were paid by a sponsor`
- L399: `binds batched presence proofs to their finalized BatchAnchor`
- L482: `represents RPC unavailability as NOT_CHECKED`

## `apps/web/tests/verify/package.test.ts`

- L29: `valid frozen package passes every local verification layer before RPC`
- L49: `one changed signed envelope byte makes the package invalid`
- L62: `changed observed RFID breaks the event-to-physical-evidence binding`
- L74: `omitting a middle event is detected`
- L87: `reordering individually valid signed events invalidates the history`
- L99: `rejects a current RFID that the history does not end with`
- L113: `verifies batched presence proofs leaf by leaf`
- L161: `checks every batched observation against its facility boundary`
- L193: `requires custody transfers to end at the current custodian`

## `chain/programs/lastro-v2/tests/custody_lineage.rs`

- L81: `a_sponsored_custody_proposal_refunds_its_rent_to_the_sponsor_only`
- L165: `custody_transfer_requires_the_named_receiver_to_sign_and_is_one_shot`
- L237: `stale_custody_proposal_is_rejected_after_state_changes`
- L279: `config_authority_rotation_needs_both_signatures`
- L527: `facility_owner_runs_a_proven_transformation_end_to_end`
- L615: `inputs_outside_the_operator_custody_cannot_be_reserved`
- L713: `a_transformation_follows_the_chain_order_inputs_then_outputs`
- L786: `a_transformation_can_be_abandoned_only_before_anything_is_consumed`

## `chain/programs/lastro-v2/tests/identity.rs`

- L234: `tag_changes_but_the_asset_identity_does_not`
- L271: `an_rfid_can_never_identify_a_second_asset`
- L320: `only_the_custodian_with_a_station_signed_payload_can_change_identity`
- L446: `presence_proofs_are_signed_by_the_custodian_or_the_authority_only`
- L483: `a_custodian_with_zero_lamports_completes_identity_and_presence_when_sponsored`
- L545: `paying_for_a_transition_grants_no_authority_over_it`
- L578: `a_reserved_input_accepts_no_station_event`

## `chain/programs/lastro-v2/tests/initial_state.rs`

- L14: `initialize_creates_config_and_all_registry_roots`
- L57: `initialize_is_reserved_to_the_program_upgrade_authority`
- L70: `initialize_is_one_time_and_does_not_replace_config`
- L84: `wrong_authority_cannot_register_asset`
- L106: `asset_registration_is_unique_and_enforces_weight_limit`
- L157: `station_registration_requires_valid_p256_key_and_derived_identity`
- L193: `station_validity_can_only_be_extended_forward_by_the_authority`
- L240: `facility_and_party_accounts_are_scoped_to_deployment`
- L304: `intent_is_bound_to_actor_and_can_be_consumed_only_once`

## `chain/programs/lastro-v2/tests/layout.rs`

- L24: `account_space_constants_match_serialized_layouts`
- L195: `asset_state_space_matches_api_decoder_layout`
- L204: `event_anchor_and_intent_offsets_match_the_api_decoders`
- L252: `session_and_batch_offsets_match_the_off_chain_decoders`
- L346: `enum_ranges_are_closed_and_stable`
- L362: `exporter_is_its_own_stable_party_role`
- L381: `asset_closed_is_terminal_for_closed_and_retired_statuses`

## `chain/programs/lastro-v2/tests/sessions.rs`

- L212: `one_signature_opens_a_session_and_batches_anchor_hundreds_of_observations`
- L249: `session_terms_are_bounded_at_opening`
- L269: `batches_must_fit_the_session_window_capacity_and_relayer`
- L292: `offline_observations_anchor_within_seven_days_and_never_after_revocation`
- L325: `only_the_custodian_closes_early_and_the_rent_returns_to_its_payer`
- L360: `a_session_requires_a_station_installed_at_an_active_facility`
- L424: `a_session_id_cannot_be_squatted_by_another_wallet`
- L490: `a_suspended_station_anchors_nothing_more`
- L539: `an_expired_station_returns_only_through_a_validity_extension`

## `crates/lastro-protocol/tests/envelope_v2.rs`

- L29: `envelope_round_trips_with_fixed_length`
- L42: `envelope_rejects_invalid_length_and_unknown_schema`
- L59: `envelope_rejects_zero_ids_and_invalid_window`
- L78: `enum_values_are_stable_and_unknown_values_are_rejected`
- L91: `domain_hashes_are_separated`

## `crates/lastro-protocol/tests/ids.rs`

- L8: `station_id_matches_domain_separated_pubkey_hash`
- L19: `invalid_p256_pubkey_length_is_rejected`

## `crates/lastro-protocol/tests/p256.rs`

- L8: `valid_compact_low_s_signature_verifies_raw_event`
- L24: `one_byte_event_change_breaks_signature`
- L44: `wrong_station_key_breaks_signature`
- L65: `high_s_signature_is_rejected`

## `crates/lastro-protocol/tests/rfid.rs`

- L11: `same_canonical_rfid_hashes_identically`
- L20: `different_canonical_rfid_values_hash_differently`
- L30: `rfid_hash_uses_exact_domain_separator`
- L43: `formatted_display_text_is_not_silently_canonical`
- L54: `canonical_rfid_is_exactly_8_bytes`

## `firmware/station/components/lastro_station/test/test_event.c`

- L5: `capture rules reject tags that violate the command`
- L23: `capture command semantics are validated before any RFID read`
- L47: `capture envelope rejects an invalid time window`

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

- L47: `signer rejects messages that are not exactly 220 bytes`
- L58: `development signer exposes the frozen compressed Station public key`
- L72: `development signature verifies as P-256 SHA-256 over the raw v2 envelope`
- L113: `signer emits canonical compact low-S signatures`
- L131: `eFuse signer preserves the Station protocol contract`

## `firmware/station/components/lastro_station/test/test_station.c`

- L35: `station boots idle`
- L44: `station command moves to wait RFID`
- L56: `station does not sign without valid RFID`
- L70: `station releases an RFID wait after its monotonic deadline`
- L94: `station rejects second command while busy`
- L111: `station new RFID hash comes from observed RFID`
- L132: `station ACK returns to idle only for matching event`
- L157: `station invalid presence observation returns to safe state`
- L180: `station accepts an identical command replay while waiting for RFID`
- L196: `station replays identical signed evidence when the same command returns before ACK`

## `firmware/station/components/lastro_station/test/test_transport.c`

- L19: `command_payload_fixture_matches_202_byte_wire_contract`
- L45: `event_ready_payload_fixture_matches_341_byte_wire_contract`
- L68: `ack_and_error_payload_offsets_are_exact`
- L97: `crc32c_castagnoli_known_vector_is_e3069283`
- L108: `transport_roundtrip_command_and_event_ready`
- L135: `transport_handles_every_single_byte_chunking`
- L157: `transport_bad_crc_is_rejected_and_next_good_frame_recovers`
- L182: `transport_oversized_payload_is_rejected_before_copy`
- L209: `transport exposes back-to-back frames from one UART chunk in order`

## `firmware/station/components/lastro_station/test/test_vectors.c`

- L24: `c_bind_envelope_equals_repository_fixture`
- L34: `c_replace_envelope_equals_repository_fixture`
- L44: `c_observe_envelope_equals_repository_fixture`

## `hardware-simulator/tests/test_protocol.py`

- L36: `test_crc32c_matches_frozen_castagnoli_vector`
- L40: `test_command_frame_round_trips_at_every_fragment_boundary`
- L50: `test_decoder_rejects_bad_crc_and_recovers_following_frame`
- L66: `test_station_id_is_derived_from_the_compressed_key`
- L70: `test_bind_replace_and_observe_envelopes_match_rust_vectors`
- L80: `test_station_rules_reject_wrong_tags`
- L91: `test_command_with_forged_context_is_rejected`

## `hardware-simulator/tests/test_repository_boundary.py`

- L8: `test_core_runtime_source_does_not_depend_on_hardware_simulator`
- L25: `test_simulator_source_stops_at_station_agent_boundary`
- L40: `test_compose_keeps_simulator_optional_and_wire_private`

## `hardware-simulator/tests/test_server.py`

- L64: `test_real_http_and_wire_servers_complete_one_origin_station_capture`

## `hardware-simulator/tests/test_station.py`

- L44: `test_station_origin_flow_emits_valid_low_s_event_and_accepts_matching_ack`
- L77: `test_observation_without_active_capture_is_ignored`
- L85: `test_fault_injection_reader_failure_fails_closed_and_returns_to_idle`
- L99: `test_corrupt_next_event_ready_crc_is_one_shot`
- L115: `test_default_signer_matches_frozen_repository_station_identity`
- L127: `test_identical_command_replay_matches_firmware_wait_states`
- L145: `test_busy_fault_emits_busy_without_consuming_a_capture`
- L156: `test_signing_failure_emits_error_and_does_not_leave_unsigned_evidence_pending`

## `services/agent/tests/command_contract.rs`

- L28: `command_carries_only_the_expected_rfid_never_the_new_one`
- L40: `identity_commands_enforce_their_rfid_context`
- L72: `state_version_zero_is_never_a_valid_successor`

## `services/agent/tests/config.rs`

- L41: `agent_requires_every_transport_identity_database_and_timing_value`
- L67: `agent_rejects_invalid_transport_identity_database_and_timing_values`
- L107: `agent_rejects_short_token_without_leaking_it`
- L122: `agent_requires_explicit_positive_station_response_timeout`
- L142: `agent_valid_configuration_preserves_serial_and_retry_values_exactly`
- L162: `the_agent_token_travels_over_https_outside_loopback`

## `services/agent/tests/domain_v2.rs`

- L34: `domain_outbox_is_idempotent_and_monotonic`
- L75: `divergent_domain_duplicate_is_rejected`

## `services/agent/tests/retry.rs`

- L156: `api_timeout_keeps_outbox_local`
- L194: `api_transport_errors_do_not_persist_url_credentials`
- L240: `api_500_retries_with_bounded_backoff`
- L314: `restart_resumes_local_rows`
- L355: `server_ack_moves_local_to_server`
- L407: `finalization_moves_server_to_finalized`

## `services/agent/tests/serial_codec.rs`

- L24: `decoder_accepts_frame_split_at_every_byte`
- L46: `decoder_rejects_bad_crc`
- L69: `decoder_rejects_oversized_payload`
- L82: `decoder_resynchronizes_after_noise`
- L98: `encoder_matches_firmware_command_vector`
- L117: `event_ready_decoder_preserves_event_signature_and_rfid`
- L134: `arbitrary_serial_bytes_never_panic`

## `services/agent/tests/serial_payload.rs`

- L28: `command_fixture_is_exactly_202_bytes_and_roundtrips_every_field`
- L51: `command_with_unknown_event_type_or_inconsistent_context_is_rejected`
- L67: `event_ready_fixture_is_exactly_341_bytes_and_preserves_signed_envelope`
- L88: `ack_fixture_binds_capture_id_to_exact_event_hash`
- L104: `error_payload_accepts_only_defined_codes_and_zero_reserved_bytes`

## `services/agent/tests/spool.rs`

- L52: `identical_duplicate_is_idempotent`
- L69: `divergent_duplicate_is_conflict`
- L89: `pending_rows_are_ordered_deterministically`
- L125: `finalized_row_is_immutable`
- L169: `sqlite_rejects_non_eight_byte_observed_rfid`

## `services/agent/tests/worker.rs`

- L299: `worker_times_out_a_station_that_never_responds`
- L344: `worker_sends_one_command_per_station_capture`
- L382: `worker_does_not_ack_before_sqlite_persist`
- L411: `worker_rejects_capture_id_mismatch`
- L451: `worker_rejects_station_pubkey_change`
- L490: `worker_never_modifies_event_bytes`
- L529: `worker_replays_ack_for_durable_local_evidence_after_restart`
- L577: `worker_replays_ack_for_server_evidence_after_restart`
- L631: `transient_command_poll_failure_does_not_terminate_worker`
- L673: `terminal_api_rejection_quarantines_durable_evidence_without_stopping_retry_processing`
- L734: `worker_refuses_to_ack_durable_evidence_from_an_unexpected_station_key`
- L771: `one_capture_breaking_the_contract_does_not_stop_the_agent`

## `services/api/tests/authorization_boundaries.rs`

- L116: `agent_endpoints_require_the_exact_bearer_token`
- L139: `public_evidence_package_endpoint_does_not_require_agent_authentication`
- L161: `wallet_private_key_fields_are_rejected_by_request_schemas`

## `services/api/tests/capture_v2.rs`

- L129: `bind_identifier_capture_reaches_finalized_evidence_package`
- L246: `only_the_canonical_custodian_can_authorize_an_identity_capture`
- L265: `capture_rules_follow_the_canonical_rfid_state`
- L290: `tampered_or_foreign_evidence_is_rejected`

## `services/api/tests/config.rs`

- L43: `startup_rejects_every_missing_required_variable_individually`
- L67: `startup_rejects_invalid_fixed_length_crypto_identifiers`
- L98: `startup_rejects_invalid_network_and_database_endpoints`
- L121: `cors_origins_are_optional_and_strictly_validated`
- L155: `priority_fee_is_optional_and_bounded`
- L176: `startup_rejects_short_demo_agent_token_without_leaking_it`
- L191: `startup_accepts_one_complete_valid_environment_without_rewriting_values`
- L212: `one_time_codes_never_go_to_the_log_of_a_public_deployment`
- L228: `the_organization_master_key_comes_from_exactly_one_source`

## `services/api/tests/errors.rs`

- L18: `validation_errors_are_400_with_stable_public_shape`
- L34: `state_conflicts_are_409_with_stable_public_shape`
- L48: `unavailable_dependencies_are_503_without_internal_details`
- L62: `internal_and_configuration_errors_do_not_leak_secrets`

## `services/api/tests/geo_v2.rs`

- L82: `a_boundary_is_immutable_and_its_commitment_is_what_the_authority_signs`

## `services/api/tests/health.rs`

- L33: `health_ok_requires_database_and_rpc`
- L49: `health_degraded_when_rpc_unavailable`
- L65: `health_degraded_when_database_unavailable`

## `services/api/tests/orgs_v2.rs`

- L219: `an_organization_runs_a_session_without_a_wallet_and_roles_bound_its_key`
- L514: `one_time_codes_are_bounded_and_do_not_reveal_accounts`
- L592: `the_sponsor_only_recognizes_wallets_the_platform_knows`

## `services/api/tests/resource_limits.rs`

- L96: `oversized_body_is_rejected_before_json_or_dependencies`
- L113: `only_operational_v2_routes_accept_bodies_above_one_kib`
- L139: `oversized_base64_evidence_is_rejected_by_the_body_limit`
- L165: `field_bounds_and_malformed_encodings_fail_before_dependencies`
- L218: `malformed_json_wrong_content_type_and_unexpected_fields_are_rejected`
- L256: `repeated_oversized_requests_remain_bounded`

## `services/api/tests/sessions_v2.rs`

- L241: `a_session_admits_offline_proofs_and_anchors_them_as_one_provable_batch`

## `services/api/tests/sponsor_v2.rs`

- L251: `the_sponsor_pays_only_for_prepared_signed_transactions_within_budget`
- L366: `sponsorship_is_refused_when_the_sponsor_is_the_deployment_authority`

## `tests/repository/test_config_syntax.py`

- L33: `test_every_json_file_parses_with_standard_json_parser`
- L41: `test_every_toml_file_parses_with_python_tomllib`
- L49: `test_every_yaml_file_parses_with_pyyaml`
- L57: `test_every_shell_script_passes_bash_syntax_check`
- L64: `test_every_python_file_compiles_without_writing_bytecode_into_repo`
- L70: `test_github_workflow_python_heredocs_compile`
- L93: `test_initialize_protocol_config_cli_loads_from_repository_root`

## `tests/repository/test_contract_schemas.py`

- L15: `test_openapi_contains_exact_v2_surface_and_no_v1_or_compliance_endpoint`
- L71: `test_openapi_transaction_contract_freezes_instruction_count_and_1232_byte_limit`
- L81: `test_openapi_submission_lifecycle_is_explicit`
- L92: `test_openapi_agent_command_cannot_supply_observed_rfid`
- L99: `test_openapi_capture_actions_are_the_three_physical_v2_operations`
- L106: `test_openapi_freezes_http_and_protocol_resource_bounds`
- L153: `test_openapi_and_the_router_describe_the_same_routes`

## `tests/repository/test_demo_preflight_contract.py`

- L6: `test_demo_preflight_is_fail_closed_and_does_not_claim_external_gates`

## `tests/repository/test_firmware_security_contract.py`

- L8: `test_station_signer_never_programs_or_changes_efuse_state`
- L24: `test_efuse_signer_is_explicit_and_fail_closed`
- L47: `test_board_entrypoint_wires_agent_transport_while_reader_protocol_stays_isolated`
- L75: `test_firmware_ci_compiles_efuse_signer_without_provisioning_commands`
- L86: `test_production_station_build_requires_secure_boot_and_flash_encryption`
- L114: `test_development_signer_is_deterministic_when_available`

## `tests/repository/test_initialize_funding.py`

- L21: `test_initializer_waits_for_the_finalized_bank_to_credit_authority`
- L30: `test_initializer_rejects_unfunded_authority_without_sending_a_transaction`

## `tests/repository/test_local_dev_environment.py`

- L18: `test_local_env_separates_container_rpc_from_browser_rpc`
- L29: `test_temporary_program_identity_restores_tracked_source_on_success_and_failure`
- L50: `test_validator_command_uses_real_test_validator_with_explicit_local_dev_bind`
- L64: `test_validator_bind_is_overridable_but_never_unspecified`
- L78: `test_local_compose_override_only_bridges_api_to_the_host_validator`
- L85: `test_local_runtime_state_is_gitignored_and_core_source_has_no_local_dev_dependency`
- L103: `test_compose_environment_overrides_hostile_shell_deployment_values`
- L136: `test_local_program_build_restores_anchor_files_and_removes_temporary_deploy_key`

## `tests/repository/test_manifest.py`

- L32: `test_manifest_covers_every_tracked_file_once_and_hashes_match`
- L47: `test_manifest_generator_ignores_untracked_local_files`

## `tests/repository/test_protocol_constants.py`

- L4: `test_envelope_length_is_consistent_across_languages_and_docs`

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
- L46: `test_static_vite_container_requires_all_public_build_arguments`
- L63: `test_chain_toolchain_is_isolated_and_pinned`
- L70: `test_litesvm_chain_tests_enable_native_precompiles`
- L75: `test_anchor_litesvm_workspace_declares_rust_test_script_and_skips_validator`
- L82: `test_program_identity_bootstrap_keeps_real_and_ci_modes_separate`
- L93: `test_full_stack_playwright_is_serial_and_has_no_retry_masking`
- L99: `test_github_actions_use_ubuntu_latest_linux_runner`
- L108: `test_github_actions_cancel_superseded_runs_per_workflow_and_ref`
- L117: `test_self_hosted_jobs_clean_workspace_before_checkout`
- L131: `test_firmware_build_does_not_run_the_checkout_inside_a_root_job_container`
- L139: `test_self_hosted_solana_jobs_use_checksum_pinned_anchor_and_solana_binaries`
- L149: `test_firmware_workflow_activates_esp_idf_environment_before_using_idf_py`
- L158: `test_maintenance_sync_only_verifies_and_never_writes_to_the_repository`
- L171: `test_maintenance_sync_fails_when_generated_outputs_are_stale`
- L182: `test_chain_resolver_lockfile_is_committed`
- L186: `test_postgres_18_compose_persists_the_real_data_root_and_binds_dev_port_to_loopback`
- L193: `test_browser_deployment_id_is_propagated_across_build_and_local_environments`
- L201: `test_local_development_requires_the_same_solana_patch_as_anchor_ci`
- L208: `test_agent_main_reconnects_after_serial_transport_failures`
- L216: `test_web_ci_program_id_matches_transaction_validation_fixture`
- L223: `test_browser_records_signed_solana_identity_before_rpc_broadcast`
- L233: `test_ci_uses_committed_resolver_lockfiles_without_fallback_resolution`
- L248: `test_dependency_bootstrap_refreshes_manifest_after_resolver_outputs`
- L256: `test_github_actions_are_pinned_to_immutable_commits`
- L267: `test_protocol_initialize_transaction_requests_explicit_compute_budget`
- L287: `test_litesvm_test_harness_boxes_large_failure_metadata`
- L293: `test_chain_tests_import_signer_trait_explicitly`
- L300: `test_secp256r1_verifier_binds_descriptor_offsets_and_sysvar_api`
- L310: `test_anchor_program_reexports_generated_account_helpers_at_crate_root`
- L323: `test_chain_sources_follow_anchor_1_2_program_api_contract`
- L336: `test_chain_rustsec_exceptions_are_narrow_and_documented`
- L354: `test_container_builds_use_committed_dependency_locks`
- L363: `test_devnet_smoke_requires_committed_program_identity`
- L369: `test_pinned_rust_action_selects_exact_toolchain_explicitly`
- L376: `test_python_ci_uses_exact_patch_without_mutable_pip_upgrade`
- L383: `test_root_node_toolchain_pins_typescript_used_by_hoisted_vue_tsc`
- L392: `test_web_typecheck_keeps_strict_source_checks_but_skips_third_party_declarations`
- L405: `test_vite_config_includes_vitest_types_for_inline_test_configuration`
- L412: `test_vue_tsc_uses_typescript_version_known_to_be_supported`
- L420: `test_node_and_npm_resolvers_are_exactly_pinned`
- L432: `test_devnet_workflow_keeps_secrets_out_of_artifacts`
- L440: `test_local_rust_commands_use_locked_dependency_graphs`
- L454: `test_web_ci_uses_only_locked_local_npm_executables`
- L468: `test_production_container_bases_are_immutable_and_runtime_has_no_package_resolution`
- L484: `test_ci_runs_repository_and_harness_tests_without_external_gate_flags`
- L496: `test_solana_cli_checks_require_exact_4_2_0_patch`

## `tests/repository/test_test_contract_quality.py`

- L15: `test_rust_contracts_define_purpose_assertion_and_failure_semantics`
- L41: `test_unity_contracts_define_purpose_assertion_and_failure_semantics`
- L58: `test_browser_contracts_define_arrange_action_assert_and_failure_semantics`
- L78: `test_hardware_and_system_pytest_contracts_define_full_execution_semantics`
- L94: `test_generated_test_index_has_no_trailing_whitespace`

## `tests/system/test_demo_stability.py`

- L23: `test_g5_three_consecutive_complete_runs`

## `tests/system/test_devnet.py`

- L23: `test_g4_devnet_flow_finalizes_and_verifies`

## `tests/system/test_full_local.py`

- L33: `test_g3_identity_and_custody_reach_finalized_canonical_state`
- L54: `test_g3_exported_package_verifies_independently_and_detects_tampering`
- L77: `test_g3_previous_custodian_can_no_longer_change_identity`
- L92: `test_g3_forged_station_evidence_is_rejected`

## `tests/system/test_support.py`

- L21: `test_local_package_verifier_accepts_genuine_and_rejects_tampered_history`
- L45: `test_pty_station_answers_the_frozen_command_with_the_frozen_envelope`
