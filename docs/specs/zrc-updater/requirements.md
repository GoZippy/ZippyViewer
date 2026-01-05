# zrc-updater Requirements

## Purpose
Secure updates for agent + desktop apps.

## Must-have
- Signed update manifests (Ed25519; optionally OS signing too)
- Delta updates optional
- Rollback safe
- Pin update keys in binaries

## Nonfunctional
- Works behind proxies
- Offline-friendly (manual update file)

