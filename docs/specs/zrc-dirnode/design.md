# zrc-dirnode Design

## Trust model
- Directory is semi-trusted for availability, not for confidentiality.
- All records signed; clients verify signatures and TTL.

## Data model
- DirRecordV1:
  - subject_id (device_id)
  - device_sign_pub (optional if already pinned; included for bootstrap)
  - endpoints hints: direct IP, relay tokens, rendezvous urls, mesh hints
  - ttl_seconds
  - signature over transcript("zrc_dir_record_v1", fields...)

## Ephemeral search mode
- DiscoveryTokenV1:
  - token_id (random)
  - subject_id
  - expires_at
  - scope (e.g., “pairing only”)
  - token signature by device or directory (choose one)
- Search index only includes token_id + minimal label (optional)
- When queried with token_id, returns record.

## Pairing flow using dirnode
- Operator gets token out-of-band (QR, short link, copy/paste)
- Operator queries dirnode with token -> obtains device public keys + rendezvous/mesh hints
- Operator sends PairRequest via mesh/rendezvous to device
- Device verifies HMAC proof from invite secret; directory never sees secret

## Open questions
- Whether tokens are signed by device or by directory.
  - Device-signed is stronger (directory can’t mint tokens).
