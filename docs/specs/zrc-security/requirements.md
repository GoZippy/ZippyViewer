# zrc-security Requirements

## Threat model (must address)
- MITM on directory/rendezvous/relay
- Impersonation of device/operator
- Replay attacks (pair/session)
- Downgrade attacks (transport/cipher suite)
- Key compromise and recovery
- Metadata privacy (who connects to whom, when)

## Must-have controls
- Identity pinning after pairing
- Signed records and tickets
- Transport pinning (QUIC cert pin or equivalent)
- E2EE session AEAD above transport
- Rate limits and lockouts
- Revocation flows:
  - remove pairing
  - revoke tickets
- Secure key storage and rotation strategy

## Compliance goals (optional)
- Audit log export
- Privacy-by-design docs

