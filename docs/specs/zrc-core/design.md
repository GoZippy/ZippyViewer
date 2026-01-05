# zrc-core Design

## Architecture
- Core is “business logic only”.
- Everything IO-related happens via adapters.

### Inputs
- Envelope bytes from mesh/rendezvous/direct/relay.

### Outputs
- Outgoing Envelope bytes + recipient id.

## Policy hooks
- PairingApprover:
  - decide approvals & permissions
- Session consent policy:
  - require_consent_each_time
  - unattended_enabled

## Transport negotiation
- Return QUIC endpoints + pinned cert.
- Later: WebRTC offer/answer, relay tokens.

## Data stores
- Invites: device_id -> invite_secret + expiry
- Pairings: (device_id, operator_id) -> pinned pubs + perms + flags
- Tickets: short-lived validation (optional cache)

## Observability
- Structured events:
  - pair_request_received
  - pair_approved/denied
  - session_init_requested
  - ticket_issued/validated
