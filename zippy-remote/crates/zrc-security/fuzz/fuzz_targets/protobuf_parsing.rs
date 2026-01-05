//! Fuzzing target for protobuf parsing.
//!
//! Requirements: 13.1

#![no_main]
use libfuzzer_sys::fuzz_target;
use zrc_proto::v1::EnvelopeV1;

fuzz_target!(|data: &[u8]| {
    // Fuzz protobuf parsing - should not panic
    let _ = EnvelopeV1::decode(data);
});
