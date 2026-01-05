# zrc-relay Requirements

## Purpose
Optional relay when NAT traversal fails:
- Forward QUIC datagrams/streams without decryption
- Provide “last resort” connectivity while keeping E2EE

## Must-have
- Relay allocation (short-lived)
- Authentication (ticket-based or relay token from session init)
- Bandwidth/connection caps per allocation
- Minimal metadata logging

## Nonfunctional
- Horizontal scaling possible
- Self-hostable
