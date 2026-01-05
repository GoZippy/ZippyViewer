# Static Analysis Configuration

## Requirements: 13.3

This document describes the static analysis tools and configuration for zrc-security.

## Cargo Audit

Configuration file: `.cargo-audit.toml`

Run security audit:
```bash
cargo audit -p zrc-security
```

## Clippy

Clippy linting is configured at the workspace level. For security-focused linting, run:

```bash
cargo clippy -p zrc-security -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic
```

### Recommended Security Lints

- `clippy::unwrap_used` - Prefer explicit error handling
- `clippy::expect_used` - Prefer explicit error handling  
- `clippy::panic` - Avoid panics in security code
- `clippy::integer_arithmetic` - Warn about integer overflow
- `clippy::cast_possible_truncation` - Warn about casts that may truncate
- `clippy::indexing_slicing` - Prefer safe indexing

## CI Integration

These tools should be integrated into CI:

1. **cargo-audit** - Run on every PR
2. **clippy** - Run with security-focused lints
3. **cargo test** - Run all unit and property tests
4. **cargo fuzz** - Run fuzzing targets nightly

## Current Status

- ✅ `.cargo-audit.toml` configured
- ✅ Clippy warnings addressed
- ✅ All code compiles without warnings (except documented exceptions)
- ⚠️ CI integration pending (to be configured in zrc-ci)
