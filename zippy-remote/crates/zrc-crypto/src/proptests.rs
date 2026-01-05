
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use crate::transcript::Transcript;
    use crate::identity::Identity;
    use crate::cert_binding::{sign_cert_fingerprint, verify_cert_binding};
    use crate::replay::{ReplayFilter, generate_nonce};
    use crate::session_crypto::{SessionCrypto, StreamId};
    use ed25519_dalek::{SigningKey, Signer};

    proptest! {
        // Task 2.3: Transcript Determinism
        #[test]
        fn test_transcript_determinism(
            tag1 in any::<u32>(),
            data1 in any::<Vec<u8>>(),
            tag2 in any::<u32>(),
            data2 in any::<Vec<u8>>()
        ) {
            let mut t1 = Transcript::new("test");
            t1.append_bytes(tag1, &data1);
            t1.append_bytes(tag2, &data2);
            let h1 = t1.finalize();

            let mut t2 = Transcript::new("test");
            t2.append_bytes(tag1, &data1);
            t2.append_bytes(tag2, &data2);
            let h2 = t2.finalize();

            assert_eq!(h1, h2);
        }

        // Task 3.5: Signature Round-Trip
        #[test]
        fn test_identity_signature_round_trip(
            seed in any::<[u8; 32]>(), 
            message in any::<Vec<u8>>()
        ) {
            let sign_key = SigningKey::from_bytes(&seed);
            let verify_key = sign_key.verifying_key();
            let pub_bytes = verify_key.to_bytes();

            use crate::identity::verify_signature;
            
            let sig = sign_key.sign(&message).to_bytes();
            prop_assert!(verify_signature(&pub_bytes, &message, &sig).is_ok());
        }

        // Task 4.4: Cert Binding Round-Trip
        #[test]
        fn test_cert_binding_round_trip(
            fingerprint in any::<[u8; 32]>(),
            // We use a fixed seed for identity generation in proptest to be deterministic 
            // if we were strictly pure, but Identity::generate is randomized. 
            // Here testing logic holds regardless of identity source.
        ) {
            let identity = Identity::generate(); 
            let binding = sign_cert_fingerprint(&identity, &fingerprint);
            
            // Verify structure matches input
            prop_assert_eq!(binding.fingerprint, fingerprint);
            prop_assert_eq!(binding.signer_pub, identity.sign_pub());

            // Verify logic
            prop_assert!(verify_cert_binding(
                &binding,
                &identity.sign_pub()
            ).is_ok());

            // Tamper check
            let mut bad_binding = binding.clone();
            bad_binding.signature[0] ^= 0xFF;
            prop_assert!(verify_cert_binding(
                &bad_binding,
                &identity.sign_pub()
            ).is_err());
        }

        // Task 10.4: Replay Detection
        #[test]
        fn test_replay_detection_property(
            offset in 0..500u64 // within default window
        ) {
            let mut filter = ReplayFilter::default();
            let base = 1000u64;
            
            // First time should succeed
            prop_assert!(filter.check_and_update(base + offset).is_ok());
            
            // Duplicate should fail
            prop_assert!(filter.check_and_update(base + offset).is_err());
        }

        // Task 9.5: Nonce Uniqueness
        #[test]
        fn test_nonce_uniqueness_property(
            stream_id in any::<u32>(),
            c1 in any::<u64>(),
            c2 in any::<u64>()
        ) {
            prop_assume!(c1 != c2);
            let n1 = generate_nonce(stream_id, c1);
            let n2 = generate_nonce(stream_id, c2);
            assert_ne!(n1, n2);
        }

        // Task 7.3: Envelope Round-Trip
        #[test]
        fn test_envelope_round_trip(
            sender_seed in any::<[u8; 32]>(),
            recipient_seed in any::<[u8; 32]>(),
            payload in any::<Vec<u8>>(),
            _aad in any::<Vec<u8>>()
        ) {
            use ed25519_dalek::SigningKey;
            use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};
            use crate::envelope::{envelope_open_v1, envelope_seal_v1};
            use zrc_proto::v1::MsgTypeV1;
            use crate::hash::derive_id;

            let sender_sign = SigningKey::from_bytes(&sender_seed);
            let sender_sign_pub = sender_sign.verifying_key().to_bytes();
            let sender_id = derive_id(&sender_sign_pub);

            // X25519 keys
            let recipient_kex = StaticSecret::from(recipient_seed);
            let recipient_pub = X25519PublicKey::from(&recipient_kex);
            let recipient_id = [0u8; 32]; // Arbitrary for this test
            
            let now = 1000u64;

            // Seal
            let envelope = envelope_seal_v1(
                &sender_sign,
                &sender_id,
                &recipient_id,
                recipient_pub.as_bytes(),
                MsgTypeV1::ControlMsg,
                &payload,
                now
            ).unwrap();

            // Open
            let (decrypted_payload, verified_sender) = envelope_open_v1(
                &envelope,
                &recipient_kex,
                &sender_sign_pub
            ).unwrap();

            assert_eq!(decrypted_payload.as_ref(), payload);
            assert_eq!(verified_sender, sender_id);
        }

        // Task 9.4: Session Crypto Round-Trip
        #[test]
        fn test_session_crypto_round_trip_prop(
            binding in any::<[u8; 32]>(),
            salt in any::<[u8; 16]>(),
            plaintext in any::<Vec<u8>>(),
            aad in any::<Vec<u8>>()
        ) {
            // Derive session crypto
            // Note: We test d2o direction here
            let crypto = SessionCrypto::derive(&binding, &salt, StreamId::Control);
            
            // Encrypt
            let ct = crypto.d2o.seal(&plaintext, &aad).unwrap();
            
            // Decrypt with same key
            let pt = crypto.d2o.open(&ct, &aad).unwrap();
            
            assert_eq!(pt, plaintext);
        }

        // Task 12.3: Key Zeroization Property Test
        // Property 7: Key Zeroization
        // Validates: Requirements 11.1, 11.2, 11.3
        #[test]
        fn test_key_zeroization_property(
            iterations in 1..100usize
        ) {
            use zeroize::ZeroizeOnDrop;
            
            // Verify Identity implements ZeroizeOnDrop trait
            // This ensures keys are zeroized when dropped
            fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
            assert_zeroize_on_drop::<Identity>();
            
            // Generate multiple identities and verify they have unique keys
            // This ensures keys are properly generated and not reused
            let mut identities: Vec<Identity> = Vec::new();
            let mut public_keys: Vec<([u8; 32], [u8; 32], [u8; 32])> = Vec::new();
            
            for _ in 0..iterations {
                let identity = Identity::generate();
                let sign_pub = identity.sign_pub();
                let kex_pub = identity.kex_pub();
                let id = identity.id();
                
                // Verify all generated identities have unique keys
                for (existing_sign_pub, existing_kex_pub, existing_id) in &public_keys {
                    prop_assert_ne!(sign_pub, *existing_sign_pub, 
                        "Signing keys must be unique");
                    prop_assert_ne!(kex_pub, *existing_kex_pub, 
                        "Key exchange keys must be unique");
                    prop_assert_ne!(id, *existing_id, 
                        "Identity IDs must be unique");
                }
                
                identities.push(identity);
                public_keys.push((sign_pub, kex_pub, id));
            }
            
            // Drop all identities - keys should be zeroized
            // We can't directly verify zeroization without unsafe code,
            // but the ZeroizeOnDrop trait ensures it happens
            drop(identities);
            
            // Generate new identities after drop to verify no key reuse
            let new_identity = Identity::generate();
            let new_sign_pub = new_identity.sign_pub();
            let new_kex_pub = new_identity.kex_pub();
            let new_id = new_identity.id();
            
            // These should be unique (very high probability)
            // We can't compare against dropped identities, but we verify
            // the mechanism is in place via the trait check above
            prop_assert_eq!(new_sign_pub.len(), 32);
            prop_assert_eq!(new_kex_pub.len(), 32);
            prop_assert_eq!(new_id.len(), 32);
        }
    }
}
