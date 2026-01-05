# zrc-dirnode Requirements

## Purpose
Decentralized “home node” directory with safe discovery:
- Invite-only by default
- Optional “searchable for brief period” mode
- Prevent MITM and hijack attempts during pairing

## Must-have functions
1) Private directory
- Store signed device records for your household/team
- Serve lookups only to authenticated clients (invite token)

2) Ephemeral discoverability
- Device can request a time-bounded “discovery token”
- Device record is searchable only while token valid
- Discovery returns only minimal pairing hints (never secrets)

3) MITM resistance
- Directory cannot substitute identity keys without detection
- Records signed by device identity key (pinned after pairing)
- Clients must verify record signatures and TTL

4) Multiple transport hints
- mesh address hints (preferred)
- rendezvous URL list (fallback)
- direct IP hints (optional)

## Nonfunctional
- Home-friendly install: single binary + config file
- Works on cheap hardware (Raspberry Pi class)
