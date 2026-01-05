# zrc-controller Requirements (CLI)

## Purpose
A power-user tool to:
- import invite (base64/QR)
- pair with a device
- start a session
- connect via QUIC (direct/relay) and drive control
- debug transports and cryptography

## Must-have
- Commands:
  - pair --invite <...>
  - session start --device <id> [--transport ladder]
  - session connect --quic <host:port> --cert <b64> --ticket <...>
- Print SAS and require confirmation when in discoverable mode
- Export pairing state + revoke pairing
