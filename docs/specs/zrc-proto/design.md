# zrc-proto Design

## Principles
- protobuf is for structure, not for cryptographic canonicalization.
- cryptographic commits/hashes use Transcript rules in zrc-crypto.

## Versioning
- messages: FooV1, enums: FooV1
- future: FooV2 alongside V1.

## Message groups
1) Identity & keys
2) Pairing
3) Session init + ticketing
4) Envelope
5) QUIC control messages (ControlMsgV1)
6) Directory records (DirRecordV1, DiscoveryTokenV1)
7) Errors (ErrorV1)

## Directory records (planned)
- Signed record with:
  - subject_id (32)
  - endpoints (optional)
  - rendezvous URLs
  - mesh address hints
  - ttl_seconds
  - signature over canonical transcript

## Open questions
- How to represent multi-monitor capture metadata (monitors list + active monitor)?
- Capability sets: granular perms vs role presets?
