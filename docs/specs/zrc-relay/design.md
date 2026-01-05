# zrc-relay Design

## Trust model
- Relay sees IP metadata and traffic volume, not plaintext.
- Session AEAD ensures confidentiality and integrity above relay.

## Operation modes
- UDP-style forwarding for QUIC datagrams (preferred)
- Stream forwarding if needed

## Tokens
- Relay token minted by device during session init:
  - contains allocation id, expiry, signed by device
- Relay validates token signature using pinned device key.

## Abuse controls
- hard caps: bytes/sec, total bytes, session duration
- optional paid/quotas later
