//! Fuzzing target for envelope decryption.
//!
//! Requirements: 13.1

#![no_main]
use libfuzzer_sys::fuzz_target;
use zrc_proto::v1::EnvelopeV1;

fuzz_target!(|data: &[u8]| {
    // Fuzz envelope parsing - should not panic
    // This tests the protobuf parsing which is the main attack surface
    let _ = EnvelopeV1::decode(data);
});
