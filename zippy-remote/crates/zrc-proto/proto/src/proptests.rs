
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use prost::Message;
    use crate::v1::{
        SessionInitRequestV1, CertBindingV1,
        InviteV1, PairRequestV1, PairReceiptV1, EndpointHintsV1, RelayTokenV1,
    };

    // Strategies for generating complex protobuf messages
    
    prop_compose! {
        fn any_session_init_req()(
            op_id in any::<Vec<u8>>(),
            dev_id in any::<Vec<u8>>(),
            sess_id in any::<Vec<u8>>(),
            caps in any::<u32>(),
            pref in 0..4i32,
            sig in any::<Vec<u8>>()
        ) -> SessionInitRequestV1 {
            SessionInitRequestV1 {
                operator_id: op_id,
                device_id: dev_id,
                session_id: sess_id,
                requested_capabilities: caps,
                transport_preference: pref, // Valid range 0-4 roughly
                operator_signature: sig,
                ticket: None, // simplified
                created_at: None,
                ticket_binding_nonce: vec![],
            }
        }
    }

    prop_compose! {
        fn any_cert_binding()(
            fp in any::<Vec<u8>>(),
            sig in any::<Vec<u8>>(),
            pk in any::<Vec<u8>>()
        ) -> CertBindingV1 {
            CertBindingV1 {
                dtls_fingerprint: fp,
                fingerprint_signature: sig,
                signer_pub: pk,
            }
        }
    }

    // Strategies for pairing messages
    // **Feature: zrc-proto, Property 1: Round-trip Encoding Consistency**
    // **Validates: Requirements 2.7, 10.6**

    prop_compose! {
        fn any_relay_token()(
            relay_id in any::<Vec<u8>>(),
            allocation_id in any::<Vec<u8>>(),
            expires_at in any::<u64>(),
            bandwidth_limit in any::<u32>(),
            signature in any::<Vec<u8>>()
        ) -> RelayTokenV1 {
            RelayTokenV1 {
                relay_id,
                allocation_id,
                expires_at,
                bandwidth_limit,
                signature,
            }
        }
    }

    prop_compose! {
        fn any_endpoint_hints()(
            direct_addrs in proptest::collection::vec(any::<String>(), 0..3),
            relay_tokens in proptest::collection::vec(any_relay_token(), 0..2),
            rendezvous_urls in proptest::collection::vec(any::<String>(), 0..3),
            mesh_hints in proptest::collection::vec(any::<String>(), 0..3)
        ) -> EndpointHintsV1 {
            EndpointHintsV1 {
                direct_addrs,
                relay_tokens,
                rendezvous_urls,
                mesh_hints,
            }
        }
    }

    prop_compose! {
        fn any_invite()(
            device_id in proptest::collection::vec(any::<u8>(), 32),
            device_sign_pub in proptest::collection::vec(any::<u8>(), 32),
            invite_secret_hash in proptest::collection::vec(any::<u8>(), 32),
            expires_at in any::<u64>(),
            transport_hints in proptest::option::of(any_endpoint_hints())
        ) -> InviteV1 {
            InviteV1 {
                device_id,
                device_sign_pub,
                invite_secret_hash,
                expires_at,
                transport_hints,
            }
        }
    }

    prop_compose! {
        fn any_pair_request()(
            operator_id in proptest::collection::vec(any::<u8>(), 32),
            operator_sign_pub in proptest::collection::vec(any::<u8>(), 32),
            operator_kex_pub in proptest::collection::vec(any::<u8>(), 32),
            invite_proof in any::<Vec<u8>>(),
            requested_permissions in any::<u32>(),
            nonce in proptest::collection::vec(any::<u8>(), 32),
            timestamp in any::<u64>()
        ) -> PairRequestV1 {
            PairRequestV1 {
                operator_id,
                operator_sign_pub,
                operator_kex_pub,
                invite_proof,
                requested_permissions,
                nonce,
                timestamp,
            }
        }
    }

    prop_compose! {
        fn any_pair_receipt()(
            device_id in proptest::collection::vec(any::<u8>(), 32),
            operator_id in proptest::collection::vec(any::<u8>(), 32),
            permissions_granted in any::<u32>(),
            paired_at in any::<u64>(),
            session_binding in any::<Vec<u8>>(),
            device_signature in proptest::collection::vec(any::<u8>(), 64)
        ) -> PairReceiptV1 {
            PairReceiptV1 {
                device_id,
                operator_id,
                permissions_granted,
                paired_at,
                session_binding,
                device_signature,
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn test_session_init_req_round_trip(req in any_session_init_req()) {
            let mut buf = Vec::new();
            req.encode(&mut buf).unwrap();
            
            let decoded = SessionInitRequestV1::decode(buf.as_slice()).unwrap();
            assert_eq!(req.operator_id, decoded.operator_id);
            assert_eq!(req.device_id, decoded.device_id);
            assert_eq!(req.session_id, decoded.session_id);
            assert_eq!(req.requested_capabilities, decoded.requested_capabilities);
        }

        #[test]
        fn test_cert_binding_round_trip(binding in any_cert_binding()) {
            let mut buf = Vec::new();
            binding.encode(&mut buf).unwrap();
            
            let decoded = CertBindingV1::decode(buf.as_slice()).unwrap();
            assert_eq!(binding.dtls_fingerprint, decoded.dtls_fingerprint);
            assert_eq!(binding.fingerprint_signature, decoded.fingerprint_signature);
            assert_eq!(binding.signer_pub, decoded.signer_pub);
        }

        // **Feature: zrc-proto, Property 1: Round-trip Encoding Consistency**
        // **Validates: Requirements 2.7, 10.6**
        // For any valid InviteV1 message, encoding to bytes then decoding back
        // SHALL produce a semantically equivalent message.
        #[test]
        fn test_invite_round_trip(invite in any_invite()) {
            let mut buf = Vec::new();
            invite.encode(&mut buf).unwrap();
            
            let decoded = InviteV1::decode(buf.as_slice()).unwrap();
            assert_eq!(invite.device_id, decoded.device_id);
            assert_eq!(invite.device_sign_pub, decoded.device_sign_pub);
            assert_eq!(invite.invite_secret_hash, decoded.invite_secret_hash);
            assert_eq!(invite.expires_at, decoded.expires_at);
            // Compare transport_hints if present
            match (&invite.transport_hints, &decoded.transport_hints) {
                (Some(orig), Some(dec)) => {
                    assert_eq!(orig.direct_addrs, dec.direct_addrs);
                    assert_eq!(orig.rendezvous_urls, dec.rendezvous_urls);
                    assert_eq!(orig.mesh_hints, dec.mesh_hints);
                    assert_eq!(orig.relay_tokens.len(), dec.relay_tokens.len());
                }
                (None, None) => {}
                _ => panic!("transport_hints mismatch"),
            }
        }

        // **Feature: zrc-proto, Property 1: Round-trip Encoding Consistency**
        // **Validates: Requirements 2.7, 10.6**
        // For any valid PairRequestV1 message, encoding to bytes then decoding back
        // SHALL produce a semantically equivalent message.
        #[test]
        fn test_pair_request_round_trip(req in any_pair_request()) {
            let mut buf = Vec::new();
            req.encode(&mut buf).unwrap();
            
            let decoded = PairRequestV1::decode(buf.as_slice()).unwrap();
            assert_eq!(req.operator_id, decoded.operator_id);
            assert_eq!(req.operator_sign_pub, decoded.operator_sign_pub);
            assert_eq!(req.operator_kex_pub, decoded.operator_kex_pub);
            assert_eq!(req.invite_proof, decoded.invite_proof);
            assert_eq!(req.requested_permissions, decoded.requested_permissions);
            assert_eq!(req.nonce, decoded.nonce);
            assert_eq!(req.timestamp, decoded.timestamp);
        }

        // **Feature: zrc-proto, Property 1: Round-trip Encoding Consistency**
        // **Validates: Requirements 2.7, 10.6**
        // For any valid PairReceiptV1 message, encoding to bytes then decoding back
        // SHALL produce a semantically equivalent message.
        #[test]
        fn test_pair_receipt_round_trip(receipt in any_pair_receipt()) {
            let mut buf = Vec::new();
            receipt.encode(&mut buf).unwrap();
            
            let decoded = PairReceiptV1::decode(buf.as_slice()).unwrap();
            assert_eq!(receipt.device_id, decoded.device_id);
            assert_eq!(receipt.operator_id, decoded.operator_id);
            assert_eq!(receipt.permissions_granted, decoded.permissions_granted);
            assert_eq!(receipt.paired_at, decoded.paired_at);
            assert_eq!(receipt.session_binding, decoded.session_binding);
            assert_eq!(receipt.device_signature, decoded.device_signature);
        }
    }
}
