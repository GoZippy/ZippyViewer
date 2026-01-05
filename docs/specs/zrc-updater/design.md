# zrc-updater Design

- TUF-like model (simplified):
  - root keys pinned
  - timestamp/snapshot/targets style metadata (optional)
- Separate update channel per platform
- Verify:
  - manifest signature
  - artifact hash
  - platform signature (MSI/pkg) if applicable

