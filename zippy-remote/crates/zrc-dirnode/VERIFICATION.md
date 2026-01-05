# Verification Status: zrc-dirnode

## Compilation Status
✅ **PASSING** - All code compiles successfully

## Test Status
✅ **PASSING** - 16 tests passing

### Unit Tests
- `test_store_round_trip` - Verifies SQLite store save/load round-trip
- `test_store_delete` - Verifies record deletion
- `test_list_expired` - Verifies expired record listing
- `test_record_store_and_retrieve` - Verifies record manager store/retrieve with signature verification
- `test_record_ttl_enforcement` - Verifies TTL enforcement
- `test_discovery_token_creation` - Verifies discovery token creation
- `test_is_discoverable` - Verifies discoverability check
- `test_token_limit` - Verifies token limit enforcement
- `test_invite_only_access` - Verifies invite-only access control
- `test_discovery_enabled_access` - Verifies discovery-enabled access mode

### Property Tests
- **Property 1: Signature Verification** (`prop_signature_verification`)
  - Validates: Requirements 1.2, 1.3
  - Verifies valid signatures succeed and invalid ones fail
  
- **Property 2: Subject ID Binding** (`prop_subject_id_binding`)
  - Validates: Requirements 1.4
  - Verifies subject_id matches SHA256(device_sign_pub)
  
- **Property 3: TTL Enforcement** (`prop_ttl_enforcement`)
  - Validates: Requirements 2.4
  - Verifies expired records are not returned
  
- **Property 4: Invite-Only Access** (`prop_invite_only_access`)
  - Validates: Requirements 3.1, 3.2
  - Verifies access control in InviteOnly mode
  
- **Property 5: Discovery Token Expiry** (`prop_discovery_token_expiry`)
  - Validates: Requirements 4.5
  - Verifies discovery tokens expire correctly
  
- **Property 6: Record Integrity** (`prop_record_integrity`)
  - Validates: Requirements 6.2
  - Verifies records are stored and retrieved without modification

## Implementation Status

### Core Features ✅
- ✅ SQLite-based record storage with schema and migrations
- ✅ Record signature verification using zrc-crypto
- ✅ Subject ID binding verification
- ✅ TTL enforcement
- ✅ Access control (InviteOnly, DiscoveryEnabled, Open modes)
- ✅ Invite token creation and validation
- ✅ Discovery token management
- ✅ HTTP API endpoints (POST/GET records, batch lookups, discovery tokens)
- ✅ Search protection (rate limiting, timing-safe responses, enumeration detection)
- ✅ Periodic record cleanup
- ✅ Configuration management (env vars, TOML files)
- ✅ Deployment artifacts (systemd unit, example config)

### Optional Features
- ✅ Web UI (Task 10) - Fully implemented:
  - Dashboard page
  - Token creation form
  - QR code generation and display
  - Token revocation UI
  - Basic authentication via admin tokens

## Running Tests

```bash
# Run all tests
cargo test --package zrc-dirnode --lib

# Run specific test
cargo test --package zrc-dirnode --lib test_record_store_and_retrieve

# Run property tests with more iterations
PROPTEST_CASES=1000 cargo test --package zrc-dirnode --lib
```

## Notes

- All property tests are implemented and passing
- Unit tests verify core functionality including signature verification and TTL enforcement
- The Web UI (Task 10) is optional and can be added later if needed
- The implementation follows privacy-first defaults with invite-only access by default
