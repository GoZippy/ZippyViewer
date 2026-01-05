//! Fuzzing target for invite parsing.
//!
//! Requirements: 13.1

#![no_main]
use libfuzzer_sys::fuzz_target;
use zrc_proto::v1::InviteV1;

fuzz_target!(|data: &[u8]| {
    // Fuzz invite parsing - should not panic
    let _ = InviteV1::decode(data);
});
