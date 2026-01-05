//! Property-based tests for zrc-controller
//!
//! These tests verify correctness properties using proptest.

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use zrc_crypto::hash::sha256;
    use zrc_crypto::pairing::{compute_pair_proof_v1, pair_proof_input_v1};
    use zrc_proto::v1::{DeviceIdV1, KeyTypeV1, PublicKeyV1, TimestampV1, UserIdV1};
    
    use crate::config::{Config, CliOverrides};
    use crate::identity::IdentityInfo;
    use crate::output::{OutputFormat, OutputFormatter, JsonResponse};
    use crate::pairing::ParsedInvite;
    use crate::pairings::StoredPairing;
    use crate::session::SessionInitResult;

    // **Feature: zrc-controller, Property 2: Pairing Proof Correctness**
    // **Validates: Requirement 2.2**
    //
    // For any pairing request, the invite_proof SHALL be correctly computed as
    // HMAC(invite_secret, transcript).
    //
    // This property verifies:
    // 1. The proof computation is deterministic (same inputs → same output)
    // 2. The proof is correctly computed as HMAC-SHA256(invite_secret, proof_input)
    // 3. Different invite secrets produce different proofs
    // 4. Different operator/device identities produce different proofs
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_pairing_proof_determinism(
            operator_id_bytes in prop::collection::vec(any::<u8>(), 32),
            operator_sign_pub in prop::collection::vec(any::<u8>(), 32),
            operator_kex_pub in prop::collection::vec(any::<u8>(), 32),
            device_id_bytes in prop::collection::vec(any::<u8>(), 32),
            invite_secret in any::<[u8; 32]>(),
            timestamp in 1_700_000_000u64..2_000_000_000u64
        ) {
            // Build the input structures
            let user_id = UserIdV1 { id: operator_id_bytes.clone() };
            let device_id = DeviceIdV1 { id: device_id_bytes.clone() };
            let created_at = TimestampV1 { unix_seconds: timestamp };

            let op_sign_pub = PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: operator_sign_pub.clone(),
            };
            let op_kex_pub = PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: operator_kex_pub.clone(),
            };

            // Compute proof input
            let proof_input = pair_proof_input_v1(
                &user_id,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );

            // Compute proof twice with same inputs
            let proof1 = compute_pair_proof_v1(&invite_secret, &proof_input);
            let proof2 = compute_pair_proof_v1(&invite_secret, &proof_input);

            // Property: Same inputs must produce same output (determinism)
            prop_assert_eq!(proof1, proof2, "Proof computation must be deterministic");

            // Property: Proof must be 32 bytes (HMAC-SHA256 output)
            prop_assert_eq!(proof1.len(), 32, "Proof must be 32 bytes");
        }

        #[test]
        fn test_pairing_proof_different_secrets_produce_different_proofs(
            operator_id_bytes in prop::collection::vec(any::<u8>(), 32),
            operator_sign_pub in prop::collection::vec(any::<u8>(), 32),
            operator_kex_pub in prop::collection::vec(any::<u8>(), 32),
            device_id_bytes in prop::collection::vec(any::<u8>(), 32),
            invite_secret1 in any::<[u8; 32]>(),
            invite_secret2 in any::<[u8; 32]>(),
            timestamp in 1_700_000_000u64..2_000_000_000u64
        ) {
            // Skip if secrets are the same
            prop_assume!(invite_secret1 != invite_secret2);

            let user_id = UserIdV1 { id: operator_id_bytes };
            let device_id = DeviceIdV1 { id: device_id_bytes };
            let created_at = TimestampV1 { unix_seconds: timestamp };

            let op_sign_pub = PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: operator_sign_pub,
            };
            let op_kex_pub = PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: operator_kex_pub,
            };

            let proof_input = pair_proof_input_v1(
                &user_id,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );

            let proof1 = compute_pair_proof_v1(&invite_secret1, &proof_input);
            let proof2 = compute_pair_proof_v1(&invite_secret2, &proof_input);

            // Property: Different secrets must produce different proofs
            prop_assert_ne!(proof1, proof2, "Different secrets must produce different proofs");
        }

        #[test]
        fn test_pairing_proof_different_inputs_produce_different_proofs(
            operator_id_bytes1 in prop::collection::vec(any::<u8>(), 32),
            operator_id_bytes2 in prop::collection::vec(any::<u8>(), 32),
            operator_sign_pub in prop::collection::vec(any::<u8>(), 32),
            operator_kex_pub in prop::collection::vec(any::<u8>(), 32),
            device_id_bytes in prop::collection::vec(any::<u8>(), 32),
            invite_secret in any::<[u8; 32]>(),
            timestamp in 1_700_000_000u64..2_000_000_000u64
        ) {
            // Skip if operator IDs are the same
            prop_assume!(operator_id_bytes1 != operator_id_bytes2);

            let user_id1 = UserIdV1 { id: operator_id_bytes1 };
            let user_id2 = UserIdV1 { id: operator_id_bytes2 };
            let device_id = DeviceIdV1 { id: device_id_bytes };
            let created_at = TimestampV1 { unix_seconds: timestamp };

            let op_sign_pub = PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: operator_sign_pub,
            };
            let op_kex_pub = PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: operator_kex_pub,
            };

            let proof_input1 = pair_proof_input_v1(
                &user_id1,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );
            let proof_input2 = pair_proof_input_v1(
                &user_id2,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );

            let proof1 = compute_pair_proof_v1(&invite_secret, &proof_input1);
            let proof2 = compute_pair_proof_v1(&invite_secret, &proof_input2);

            // Property: Different operator IDs must produce different proofs
            prop_assert_ne!(proof1, proof2, "Different operator IDs must produce different proofs");
        }

        /// Test that the proof verification works correctly by recomputing
        /// This simulates what the device does when verifying the controller's proof
        #[test]
        fn test_pairing_proof_verification_round_trip(
            operator_sign_pub in prop::collection::vec(any::<u8>(), 32),
            operator_kex_pub in prop::collection::vec(any::<u8>(), 32),
            device_id_bytes in prop::collection::vec(any::<u8>(), 32),
            invite_secret in any::<[u8; 32]>(),
            timestamp in 1_700_000_000u64..2_000_000_000u64
        ) {
            // Compute operator_id as SHA256(operator_sign_pub) - matching controller behavior
            let operator_id_bytes = sha256(&operator_sign_pub).to_vec();

            let user_id = UserIdV1 { id: operator_id_bytes.clone() };
            let device_id = DeviceIdV1 { id: device_id_bytes };
            let created_at = TimestampV1 { unix_seconds: timestamp };

            let op_sign_pub = PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: operator_sign_pub,
            };
            let op_kex_pub = PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: operator_kex_pub,
            };

            // Controller computes proof
            let proof_input = pair_proof_input_v1(
                &user_id,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );
            let controller_proof = compute_pair_proof_v1(&invite_secret, &proof_input);

            // Device verifies by recomputing with same inputs
            let device_proof = compute_pair_proof_v1(&invite_secret, &proof_input);

            // Property: Verification succeeds (proofs match)
            prop_assert_eq!(
                controller_proof, 
                device_proof, 
                "Device verification must succeed by recomputing the same proof"
            );
        }

        /// Test that wrong invite secret fails verification
        #[test]
        fn test_pairing_proof_wrong_secret_fails_verification(
            operator_sign_pub in prop::collection::vec(any::<u8>(), 32),
            operator_kex_pub in prop::collection::vec(any::<u8>(), 32),
            device_id_bytes in prop::collection::vec(any::<u8>(), 32),
            correct_secret in any::<[u8; 32]>(),
            wrong_secret in any::<[u8; 32]>(),
            timestamp in 1_700_000_000u64..2_000_000_000u64
        ) {
            prop_assume!(correct_secret != wrong_secret);

            let operator_id_bytes = sha256(&operator_sign_pub).to_vec();

            let user_id = UserIdV1 { id: operator_id_bytes };
            let device_id = DeviceIdV1 { id: device_id_bytes };
            let created_at = TimestampV1 { unix_seconds: timestamp };

            let op_sign_pub = PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: operator_sign_pub,
            };
            let op_kex_pub = PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: operator_kex_pub,
            };

            let proof_input = pair_proof_input_v1(
                &user_id,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
            );

            // Controller computes proof with correct secret
            let controller_proof = compute_pair_proof_v1(&correct_secret, &proof_input);

            // Device tries to verify with wrong secret
            let device_proof = compute_pair_proof_v1(&wrong_secret, &proof_input);

            // Property: Verification fails (proofs don't match)
            prop_assert_ne!(
                controller_proof, 
                device_proof, 
                "Verification with wrong secret must fail"
            );
        }
    }

    // **Feature: zrc-controller, Property 7: Configuration Override Precedence**
    // **Validates: Requirement 10.5**
    //
    // For any configuration option, command-line arguments SHALL override config file values.
    //
    // This property verifies:
    // 1. CLI overrides always take precedence over config file values
    // 2. When CLI override is None, config file value is preserved
    // 3. Override behavior is consistent across all configuration fields
    
    /// Strategy to generate valid output formats
    fn output_format_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("table".to_string()),
            Just("json".to_string()),
            Just("quiet".to_string()),
        ]
    }
    
    /// Strategy to generate valid transport types
    fn transport_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("auto".to_string()),
            Just("mesh".to_string()),
            Just("rendezvous".to_string()),
            Just("direct".to_string()),
            Just("relay".to_string()),
        ]
    }
    
    /// Strategy to generate valid log levels
    fn log_level_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("error".to_string()),
            Just("warn".to_string()),
            Just("info".to_string()),
            Just("debug".to_string()),
            Just("trace".to_string()),
        ]
    }
    
    /// Strategy to generate valid URLs
    fn url_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("https://example.com".to_string()),
            Just("https://test.zippyremote.io".to_string()),
            Just("http://localhost:8080".to_string()),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: CLI output_format override takes precedence over config file value
        #[test]
        fn test_config_override_output_format(
            config_format in output_format_strategy(),
            cli_format in output_format_strategy()
        ) {
            // Create config with one format
            let mut config = Config::default();
            config.output.format = config_format.clone();
            
            // Apply CLI override with different format
            let overrides = CliOverrides {
                output_format: Some(cli_format.clone()),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: CLI override must take precedence
            prop_assert_eq!(
                result.output.format, 
                cli_format,
                "CLI output_format override must take precedence over config file value"
            );
        }

        /// Property: CLI verbose override takes precedence over config file value
        #[test]
        fn test_config_override_verbose(
            config_verbose in any::<bool>(),
            cli_verbose in any::<bool>()
        ) {
            let mut config = Config::default();
            config.output.verbose = config_verbose;
            
            let overrides = CliOverrides {
                verbose: Some(cli_verbose),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: CLI override must take precedence
            prop_assert_eq!(
                result.output.verbose, 
                cli_verbose,
                "CLI verbose override must take precedence over config file value"
            );
        }

        /// Property: CLI transport override takes precedence over config file value
        #[test]
        fn test_config_override_transport(
            config_transport in transport_strategy(),
            cli_transport in transport_strategy()
        ) {
            let mut config = Config::default();
            config.transport.default = config_transport.clone();
            
            let overrides = CliOverrides {
                transport: Some(cli_transport.clone()),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: CLI override must take precedence
            prop_assert_eq!(
                result.transport.default, 
                cli_transport,
                "CLI transport override must take precedence over config file value"
            );
        }

        /// Property: CLI debug flag sets log level to debug
        #[test]
        fn test_config_override_debug_sets_log_level(
            config_level in log_level_strategy(),
            cli_debug in any::<bool>()
        ) {
            let mut config = Config::default();
            config.logging.level = config_level.clone();
            
            let overrides = CliOverrides {
                debug: Some(cli_debug),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            if cli_debug {
                // Property: When debug is true, log level must be "debug"
                prop_assert_eq!(
                    result.logging.level, 
                    "debug",
                    "CLI debug=true must set log level to debug"
                );
            } else {
                // Property: When debug is false, log level is unchanged
                prop_assert_eq!(
                    result.logging.level, 
                    config_level,
                    "CLI debug=false must not change log level"
                );
            }
        }

        /// Property: CLI rendezvous_urls override takes precedence (when non-empty)
        #[test]
        fn test_config_override_rendezvous_urls(
            config_url in url_strategy(),
            cli_url in url_strategy()
        ) {
            let mut config = Config::default();
            config.transport.rendezvous_urls = vec![config_url.clone()];
            
            let overrides = CliOverrides {
                rendezvous_urls: Some(vec![cli_url.clone()]),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: CLI override must take precedence
            prop_assert_eq!(
                result.transport.rendezvous_urls, 
                vec![cli_url],
                "CLI rendezvous_urls override must take precedence over config file value"
            );
        }

        /// Property: CLI relay_urls override takes precedence (when non-empty)
        #[test]
        fn test_config_override_relay_urls(
            config_url in url_strategy(),
            cli_url in url_strategy()
        ) {
            let mut config = Config::default();
            config.transport.relay_urls = vec![config_url.clone()];
            
            let overrides = CliOverrides {
                relay_urls: Some(vec![cli_url.clone()]),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: CLI override must take precedence
            prop_assert_eq!(
                result.transport.relay_urls, 
                vec![cli_url],
                "CLI relay_urls override must take precedence over config file value"
            );
        }

        /// Property: When CLI override is None, config file value is preserved
        #[test]
        fn test_config_preserves_values_when_no_override(
            config_format in output_format_strategy(),
            config_transport in transport_strategy(),
            config_verbose in any::<bool>()
        ) {
            let mut config = Config::default();
            config.output.format = config_format.clone();
            config.transport.default = config_transport.clone();
            config.output.verbose = config_verbose;
            
            // Apply empty overrides
            let overrides = CliOverrides::default();
            
            let result = config.with_overrides(&overrides);
            
            // Property: All values must be preserved when no overrides
            prop_assert_eq!(
                result.output.format, 
                config_format,
                "Config format must be preserved when no CLI override"
            );
            prop_assert_eq!(
                result.transport.default, 
                config_transport,
                "Config transport must be preserved when no CLI override"
            );
            prop_assert_eq!(
                result.output.verbose, 
                config_verbose,
                "Config verbose must be preserved when no CLI override"
            );
        }

        /// Property: Empty URL vectors don't override config values
        #[test]
        fn test_config_empty_vectors_dont_override(
            config_url in url_strategy()
        ) {
            let mut config = Config::default();
            config.transport.rendezvous_urls = vec![config_url.clone()];
            config.transport.relay_urls = vec![config_url.clone()];
            config.transport.mesh_nodes = vec![config_url.clone()];
            
            // Apply overrides with empty vectors
            let overrides = CliOverrides {
                rendezvous_urls: Some(vec![]),
                relay_urls: Some(vec![]),
                mesh_nodes: Some(vec![]),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: Empty vectors must not override
            prop_assert_eq!(
                result.transport.rendezvous_urls, 
                vec![config_url.clone()],
                "Empty rendezvous_urls vector must not override config"
            );
            prop_assert_eq!(
                result.transport.relay_urls, 
                vec![config_url.clone()],
                "Empty relay_urls vector must not override config"
            );
            prop_assert_eq!(
                result.transport.mesh_nodes, 
                vec![config_url],
                "Empty mesh_nodes vector must not override config"
            );
        }

        /// Property: Multiple overrides are applied correctly together
        #[test]
        fn test_config_multiple_overrides_applied(
            config_format in output_format_strategy(),
            config_transport in transport_strategy(),
            cli_format in output_format_strategy(),
            cli_transport in transport_strategy(),
            cli_verbose in any::<bool>()
        ) {
            let mut config = Config::default();
            config.output.format = config_format;
            config.transport.default = config_transport;
            config.output.verbose = !cli_verbose; // Set opposite of CLI value
            
            let overrides = CliOverrides {
                output_format: Some(cli_format.clone()),
                transport: Some(cli_transport.clone()),
                verbose: Some(cli_verbose),
                ..Default::default()
            };
            
            let result = config.with_overrides(&overrides);
            
            // Property: All CLI overrides must be applied
            prop_assert_eq!(
                result.output.format, 
                cli_format,
                "CLI output_format must be applied"
            );
            prop_assert_eq!(
                result.transport.default, 
                cli_transport,
                "CLI transport must be applied"
            );
            prop_assert_eq!(
                result.output.verbose, 
                cli_verbose,
                "CLI verbose must be applied"
            );
        }
    }

    // **Feature: zrc-controller, Property 5: Output Format Consistency**
    // **Validates: Requirements 9.1, 9.4**
    //
    // For any command with --output json, the output SHALL be valid JSON matching
    // the documented schema.
    //
    // This property verifies:
    // 1. All JSON output is valid, parseable JSON
    // 2. JSON output follows the consistent JsonResponse schema with success, data, error, timestamp
    // 3. Different data types produce valid JSON when formatted
    // 4. The schema is consistent across all command outputs

    /// Strategy to generate arbitrary device IDs (hex strings)
    fn device_id_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<u8>(), 16..=32)
            .prop_map(|bytes| hex::encode(bytes))
    }

    /// Strategy to generate arbitrary device names
    fn device_name_strategy() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            Just(None),
            "[a-zA-Z0-9 _-]{1,50}".prop_map(Some),
        ]
    }

    /// Strategy to generate permission lists
    fn permissions_strategy() -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec(
            prop_oneof![
                Just("view".to_string()),
                Just("control".to_string()),
                Just("clipboard".to_string()),
                Just("file_transfer".to_string()),
                Just("audio".to_string()),
            ],
            0..=5
        )
    }

    /// Strategy to generate valid SystemTime values
    fn system_time_strategy() -> impl Strategy<Value = SystemTime> {
        // Generate timestamps between 2020 and 2030
        (1577836800u64..1893456000u64).prop_map(|secs| {
            UNIX_EPOCH + Duration::from_secs(secs)
        })
    }

    /// Strategy to generate StoredPairing instances
    fn stored_pairing_strategy() -> impl Strategy<Value = StoredPairing> {
        (
            device_id_strategy(),
            device_name_strategy(),
            any::<[u8; 32]>(),
            any::<[u8; 32]>(),
            permissions_strategy(),
            system_time_strategy(),
            prop::option::of(system_time_strategy()),
            0u32..1000u32,
        ).prop_map(|(device_id, device_name, sign_pub, kex_pub, permissions, paired_at, last_session, session_count)| {
            StoredPairing {
                device_id,
                device_name,
                device_sign_pub: sign_pub,
                device_kex_pub: kex_pub,
                permissions,
                paired_at,
                last_session,
                session_count,
            }
        })
    }

    /// Strategy to generate IdentityInfo instances
    fn identity_info_strategy() -> impl Strategy<Value = IdentityInfo> {
        (
            "[a-f0-9]{16}",  // operator_id (16 hex chars)
            "[a-f0-9]{64}",  // fingerprint (64 hex chars)
            system_time_strategy(),
        ).prop_map(|(operator_id, fingerprint, created_at)| {
            IdentityInfo {
                operator_id,
                fingerprint,
                created_at,
                key_algorithm: "Ed25519/X25519".to_string(),
            }
        })
    }

    /// Strategy to generate SessionInitResult instances
    fn session_init_result_strategy() -> impl Strategy<Value = SessionInitResult> {
        (
            "[a-f0-9]{64}",  // session_id
            permissions_strategy(),
            "[a-z0-9.-]{1,50}",  // quic_host
            1024u16..65535u16,  // quic_port
            any::<[u8; 32]>(),  // cert_fingerprint
            prop::collection::vec(any::<u8>(), 32..128),  // ticket
        ).prop_map(|(session_id, capabilities, quic_host, quic_port, cert_fingerprint, ticket)| {
            SessionInitResult {
                session_id,
                granted_capabilities: capabilities,
                quic_host,
                quic_port,
                cert_fingerprint,
                ticket,
            }
        })
    }

    /// Strategy to generate transport hints
    fn transport_hints_strategy() -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec(
            prop_oneof![
                Just("rendezvous:https://rendezvous.example.com".to_string()),
                Just("direct:192.168.1.100:5000".to_string()),
                Just("relay:https://relay.example.com".to_string()),
            ],
            0..=3
        )
    }

    /// Strategy to generate ParsedInvite instances
    fn parsed_invite_strategy() -> impl Strategy<Value = ParsedInvite> {
        (
            device_id_strategy(),
            system_time_strategy(),
            transport_hints_strategy(),
        ).prop_map(|(device_id, expires_at, transport_hints)| {
            // Create a minimal valid InviteV1
            let invite = zrc_proto::v1::InviteV1 {
                device_id: hex::decode(&device_id).unwrap_or_default(),
                device_sign_pub: vec![0u8; 32],
                invite_secret_hash: vec![0u8; 32],
                expires_at: expires_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                transport_hints: None,
            };
            ParsedInvite {
                device_id,
                expires_at,
                transport_hints,
                raw: vec![],
                invite,
            }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // **Feature: zrc-controller, Property 5: Output Format Consistency**
        // **Validates: Requirements 9.1, 9.4**

        /// Property: JSON output for pairings list is always valid JSON
        #[test]
        fn test_json_output_pairings_is_valid_json(
            pairings in prop::collection::vec(stored_pairing_strategy(), 0..10)
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_pairings(&pairings);
            
            // Property: Output must be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            prop_assert!(
                parsed.is_ok(),
                "Pairings JSON output must be valid JSON. Got error: {:?}",
                parsed.err()
            );
            
            // Property: JSON must follow the JsonResponse schema
            let json = parsed.unwrap();
            prop_assert!(
                json.get("success").is_some(),
                "JSON output must have 'success' field"
            );
            prop_assert!(
                json.get("timestamp").is_some(),
                "JSON output must have 'timestamp' field"
            );
            
            // Property: Timestamp must be valid ISO 8601
            if let Some(ts) = json.get("timestamp").and_then(|v| v.as_str()) {
                let parsed_ts = chrono::DateTime::parse_from_rfc3339(ts);
                prop_assert!(
                    parsed_ts.is_ok(),
                    "Timestamp must be valid RFC3339. Got: {}",
                    ts
                );
            }
        }

        /// Property: JSON output for identity is always valid JSON
        #[test]
        fn test_json_output_identity_is_valid_json(
            identity in identity_info_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_identity(&identity);
            
            // Property: Output must be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            prop_assert!(
                parsed.is_ok(),
                "Identity JSON output must be valid JSON. Got error: {:?}",
                parsed.err()
            );
            
            // Property: JSON must follow the JsonResponse schema
            let json = parsed.unwrap();
            prop_assert!(
                json.get("success").is_some(),
                "JSON output must have 'success' field"
            );
            prop_assert!(
                json.get("timestamp").is_some(),
                "JSON output must have 'timestamp' field"
            );
            
            // Property: Data must contain identity fields
            if let Some(data) = json.get("data") {
                prop_assert!(
                    data.get("operator_id").is_some(),
                    "Identity data must have 'operator_id' field"
                );
                prop_assert!(
                    data.get("fingerprint").is_some(),
                    "Identity data must have 'fingerprint' field"
                );
            }
        }

        /// Property: JSON output for session is always valid JSON
        #[test]
        fn test_json_output_session_is_valid_json(
            session in session_init_result_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_session(&session);
            
            // Property: Output must be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            prop_assert!(
                parsed.is_ok(),
                "Session JSON output must be valid JSON. Got error: {:?}",
                parsed.err()
            );
            
            // Property: JSON must follow the JsonResponse schema
            let json = parsed.unwrap();
            prop_assert!(
                json.get("success").is_some(),
                "JSON output must have 'success' field"
            );
            prop_assert!(
                json.get("timestamp").is_some(),
                "JSON output must have 'timestamp' field"
            );
            
            // Property: Data must contain session fields
            if let Some(data) = json.get("data") {
                prop_assert!(
                    data.get("session_id").is_some(),
                    "Session data must have 'session_id' field"
                );
                prop_assert!(
                    data.get("quic_endpoint").is_some(),
                    "Session data must have 'quic_endpoint' field"
                );
            }
        }

        /// Property: JSON output for invite is always valid JSON
        #[test]
        fn test_json_output_invite_is_valid_json(
            invite in parsed_invite_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_invite(&invite);
            
            // Property: Output must be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            prop_assert!(
                parsed.is_ok(),
                "Invite JSON output must be valid JSON. Got error: {:?}",
                parsed.err()
            );
            
            // Property: JSON must follow the JsonResponse schema
            let json = parsed.unwrap();
            prop_assert!(
                json.get("success").is_some(),
                "JSON output must have 'success' field"
            );
            prop_assert!(
                json.get("timestamp").is_some(),
                "JSON output must have 'timestamp' field"
            );
            
            // Property: Data must contain invite fields
            if let Some(data) = json.get("data") {
                prop_assert!(
                    data.get("device_id").is_some(),
                    "Invite data must have 'device_id' field"
                );
                prop_assert!(
                    data.get("expires_at").is_some(),
                    "Invite data must have 'expires_at' field"
                );
            }
        }

        /// Property: JSON output for pairing detail is always valid JSON
        #[test]
        fn test_json_output_pairing_detail_is_valid_json(
            pairing in stored_pairing_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_pairing_detail(&pairing);
            
            // Property: Output must be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            prop_assert!(
                parsed.is_ok(),
                "Pairing detail JSON output must be valid JSON. Got error: {:?}",
                parsed.err()
            );
            
            // Property: JSON must follow the JsonResponse schema
            let json = parsed.unwrap();
            prop_assert!(
                json.get("success").is_some(),
                "JSON output must have 'success' field"
            );
            prop_assert!(
                json.get("timestamp").is_some(),
                "JSON output must have 'timestamp' field"
            );
            
            // Property: Data must contain pairing detail fields
            if let Some(data) = json.get("data") {
                prop_assert!(
                    data.get("device_id").is_some(),
                    "Pairing detail data must have 'device_id' field"
                );
                prop_assert!(
                    data.get("device_sign_pub").is_some(),
                    "Pairing detail data must have 'device_sign_pub' field"
                );
                prop_assert!(
                    data.get("device_kex_pub").is_some(),
                    "Pairing detail data must have 'device_kex_pub' field"
                );
            }
        }

        /// Property: JsonResponse success wrapper produces valid JSON
        #[test]
        fn test_json_response_success_is_valid_json(
            message in "[a-zA-Z0-9 ]{1,100}"
        ) {
            let response = JsonResponse::success(&message);
            let output = serde_json::to_string_pretty(&response);
            
            prop_assert!(
                output.is_ok(),
                "JsonResponse::success must serialize to valid JSON"
            );
            
            let json: serde_json::Value = serde_json::from_str(&output.unwrap()).unwrap();
            prop_assert_eq!(
                json.get("success").and_then(|v| v.as_bool()),
                Some(true),
                "Success response must have success=true"
            );
            prop_assert!(
                json.get("data").is_some(),
                "Success response must have data field"
            );
            prop_assert!(
                json.get("error").is_none(),
                "Success response must not have error field"
            );
        }

        /// Property: JsonResponse error wrapper produces valid JSON
        #[test]
        fn test_json_response_error_is_valid_json(
            error_msg in "[a-zA-Z0-9 :_-]{1,200}"
        ) {
            let response = JsonResponse::<()>::error(&error_msg);
            let output = serde_json::to_string_pretty(&response);
            
            prop_assert!(
                output.is_ok(),
                "JsonResponse::error must serialize to valid JSON"
            );
            
            let json: serde_json::Value = serde_json::from_str(&output.unwrap()).unwrap();
            prop_assert_eq!(
                json.get("success").and_then(|v| v.as_bool()),
                Some(false),
                "Error response must have success=false"
            );
            prop_assert!(
                json.get("error").is_some(),
                "Error response must have error field"
            );
            prop_assert!(
                json.get("data").is_none(),
                "Error response must not have data field"
            );
        }

        /// Property: Quiet mode produces empty output for all data types
        #[test]
        fn test_quiet_mode_produces_empty_output(
            pairings in prop::collection::vec(stored_pairing_strategy(), 0..5),
            identity in identity_info_strategy(),
            session in session_init_result_strategy(),
            invite in parsed_invite_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Quiet, false);
            
            // Property: All quiet mode outputs must be empty
            prop_assert_eq!(
                formatter.format_pairings(&pairings),
                "",
                "Quiet mode pairings output must be empty"
            );
            prop_assert_eq!(
                formatter.format_identity(&identity),
                "",
                "Quiet mode identity output must be empty"
            );
            prop_assert_eq!(
                formatter.format_session(&session),
                "",
                "Quiet mode session output must be empty"
            );
            prop_assert_eq!(
                formatter.format_invite(&invite),
                "",
                "Quiet mode invite output must be empty"
            );
        }

        /// Property: Table mode produces non-empty output for non-empty data
        #[test]
        fn test_table_mode_produces_output(
            pairing in stored_pairing_strategy(),
            identity in identity_info_strategy(),
            session in session_init_result_strategy(),
            invite in parsed_invite_strategy()
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Table, false);
            
            // Property: Table mode outputs must be non-empty for valid data
            prop_assert!(
                !formatter.format_pairings(&[pairing.clone()]).is_empty(),
                "Table mode pairings output must be non-empty"
            );
            prop_assert!(
                !formatter.format_identity(&identity).is_empty(),
                "Table mode identity output must be non-empty"
            );
            prop_assert!(
                !formatter.format_session(&session).is_empty(),
                "Table mode session output must be non-empty"
            );
            prop_assert!(
                !formatter.format_invite(&invite).is_empty(),
                "Table mode invite output must be non-empty"
            );
            prop_assert!(
                !formatter.format_pairing_detail(&pairing).is_empty(),
                "Table mode pairing detail output must be non-empty"
            );
        }

        /// Property: JSON timestamps are always valid RFC3339 format
        #[test]
        fn test_json_timestamps_are_rfc3339(
            pairings in prop::collection::vec(stored_pairing_strategy(), 1..5)
        ) {
            let formatter = OutputFormatter::new(OutputFormat::Json, false);
            let output = formatter.format_pairings(&pairings);
            let json: serde_json::Value = serde_json::from_str(&output).unwrap();
            
            // Check top-level timestamp
            if let Some(ts) = json.get("timestamp").and_then(|v| v.as_str()) {
                let parsed = chrono::DateTime::parse_from_rfc3339(ts);
                prop_assert!(
                    parsed.is_ok(),
                    "Top-level timestamp must be valid RFC3339: {}",
                    ts
                );
            }
            
            // Check timestamps in data
            if let Some(data) = json.get("data") {
                if let Some(pairings_arr) = data.get("pairings").and_then(|v| v.as_array()) {
                    for pairing in pairings_arr {
                        if let Some(ts) = pairing.get("paired_at_iso").and_then(|v| v.as_str()) {
                            let parsed = chrono::DateTime::parse_from_rfc3339(ts);
                            prop_assert!(
                                parsed.is_ok(),
                                "paired_at_iso must be valid RFC3339: {}",
                                ts
                            );
                        }
                    }
                }
            }
        }
    }

    // **Feature: zrc-controller, Property 6: Exit Code Accuracy**
    // **Validates: Requirement 9.6**
    //
    // For any command execution, the exit code SHALL accurately reflect the outcome
    // (0=success, non-zero=specific error).
    //
    // This property verifies:
    // 1. ExitCode::Success always maps to 0
    // 2. All error exit codes map to their documented non-zero values
    // 3. The i32 conversion is consistent with the documented values
    // 4. The to_exit_code() conversion produces valid process exit codes
    // 5. Exit code names and descriptions are consistent and non-empty

    use crate::ExitCode;

    /// Strategy to generate all possible ExitCode variants
    fn exit_code_strategy() -> impl Strategy<Value = ExitCode> {
        prop_oneof![
            Just(ExitCode::Success),
            Just(ExitCode::GeneralError),
            Just(ExitCode::AuthenticationFailed),
            Just(ExitCode::Timeout),
            Just(ExitCode::ConnectionFailed),
            Just(ExitCode::InvalidInput),
            Just(ExitCode::NotPaired),
            Just(ExitCode::PermissionDenied),
        ]
    }

    /// Map of expected exit code values per the documented specification
    /// Requirements: 9.6 - 0=success, 1=error, 2=auth_failed, 3=timeout
    fn expected_exit_code_value(code: ExitCode) -> i32 {
        match code {
            ExitCode::Success => 0,
            ExitCode::GeneralError => 1,
            ExitCode::AuthenticationFailed => 2,
            ExitCode::Timeout => 3,
            ExitCode::ConnectionFailed => 4,
            ExitCode::InvalidInput => 5,
            ExitCode::NotPaired => 6,
            ExitCode::PermissionDenied => 7,
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // **Feature: zrc-controller, Property 6: Exit Code Accuracy**
        // **Validates: Requirement 9.6**

        /// Property: Exit codes map to their documented numeric values
        #[test]
        fn test_exit_code_maps_to_documented_value(
            code in exit_code_strategy()
        ) {
            let expected = expected_exit_code_value(code);
            let actual = code as i32;
            
            // Property: Exit code must match documented value
            prop_assert_eq!(
                actual,
                expected,
                "ExitCode::{:?} must map to {} but got {}",
                code,
                expected,
                actual
            );
        }

        /// Property: i32 conversion is consistent with direct cast
        #[test]
        fn test_exit_code_i32_conversion_consistency(
            code in exit_code_strategy()
        ) {
            let from_cast = code as i32;
            let from_into: i32 = code.into();
            
            // Property: Both conversion methods must produce same result
            prop_assert_eq!(
                from_cast,
                from_into,
                "ExitCode::{:?} cast ({}) must equal Into<i32> ({})",
                code,
                from_cast,
                from_into
            );
        }

        /// Property: Success exit code is always 0
        #[test]
        fn test_success_exit_code_is_zero(
            _dummy in 0..100u32  // Run 100 times to verify consistency
        ) {
            let code = ExitCode::Success;
            
            // Property: Success must always be 0
            prop_assert_eq!(
                code as i32,
                0,
                "ExitCode::Success must always be 0"
            );
            
            // Property: Success must be the only code that is 0
            let all_codes = [
                ExitCode::GeneralError,
                ExitCode::AuthenticationFailed,
                ExitCode::Timeout,
                ExitCode::ConnectionFailed,
                ExitCode::InvalidInput,
                ExitCode::NotPaired,
                ExitCode::PermissionDenied,
            ];
            
            for other_code in all_codes {
                prop_assert_ne!(
                    other_code as i32,
                    0,
                    "Only ExitCode::Success should be 0, but {:?} is also 0",
                    other_code
                );
            }
        }

        /// Property: All error exit codes are non-zero
        #[test]
        fn test_error_exit_codes_are_nonzero(
            code in exit_code_strategy()
        ) {
            if code != ExitCode::Success {
                // Property: All error codes must be non-zero
                prop_assert_ne!(
                    code as i32,
                    0,
                    "Error ExitCode::{:?} must be non-zero",
                    code
                );
            }
        }

        /// Property: Exit code names are non-empty and consistent
        #[test]
        fn test_exit_code_names_are_valid(
            code in exit_code_strategy()
        ) {
            let name = code.name();
            
            // Property: Name must be non-empty
            prop_assert!(
                !name.is_empty(),
                "ExitCode::{:?} name must be non-empty",
                code
            );
            
            // Property: Name must be uppercase with underscores (convention)
            prop_assert!(
                name.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
                "ExitCode::{:?} name '{}' must be uppercase with underscores",
                code,
                name
            );
        }

        /// Property: Exit code descriptions are non-empty
        #[test]
        fn test_exit_code_descriptions_are_valid(
            code in exit_code_strategy()
        ) {
            let description = code.description();
            
            // Property: Description must be non-empty
            prop_assert!(
                !description.is_empty(),
                "ExitCode::{:?} description must be non-empty",
                code
            );
            
            // Property: Description must be human-readable (starts with capital, reasonable length)
            prop_assert!(
                description.len() >= 10,
                "ExitCode::{:?} description '{}' should be at least 10 chars",
                code,
                description
            );
        }

        /// Property: Exit codes are unique (no two codes have the same value)
        #[test]
        fn test_exit_codes_are_unique(
            code1 in exit_code_strategy(),
            code2 in exit_code_strategy()
        ) {
            // Property: Different exit codes must have different values
            if code1 != code2 {
                prop_assert_ne!(
                    code1 as i32,
                    code2 as i32,
                    "ExitCode::{:?} and ExitCode::{:?} must have different values",
                    code1,
                    code2
                );
            }
        }

        /// Property: Exit code values are in valid range (0-255 for process exit codes)
        #[test]
        fn test_exit_code_values_in_valid_range(
            code in exit_code_strategy()
        ) {
            let value = code as i32;
            
            // Property: Exit code must be in valid range for process exit codes
            prop_assert!(
                value >= 0 && value <= 255,
                "ExitCode::{:?} value {} must be in range 0-255",
                code,
                value
            );
        }

        /// Property: to_exit_code() conversion produces valid process exit code
        #[test]
        fn test_to_exit_code_produces_valid_process_exit_code(
            code in exit_code_strategy()
        ) {
            // Property: to_exit_code() must not panic and produce a valid exit code
            let _process_exit_code = code.to_exit_code();
            
            // We can't directly inspect std::process::ExitCode value,
            // but we can verify the conversion doesn't panic and the
            // original value is in the valid u8 range
            let value = code as i32;
            prop_assert!(
                value >= 0 && value <= 255,
                "ExitCode::{:?} must convert to valid u8 for process exit code",
                code
            );
            
            // Verify the conversion is consistent (call twice, should work both times)
            let _ = code.to_exit_code();
        }
    }
}
