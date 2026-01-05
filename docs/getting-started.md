# Getting Started

This guide will help you install and configure ZippyViewer for remote desktop access.

## Prerequisites

- **Rust 1.75+** (for building from source)
- **Protocol Buffers compiler** (`protoc`)
- Platform-specific SDKs for mobile builds

## Installation

### From Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/GoZippy/ZippyViewer/releases) page.

### Building from Source

```bash
# Clone the repository
git clone https://github.com/GoZippy/ZippyViewer.git
cd ZippyViewer/zippy-remote

# Build all components
cargo build --release

# Binaries are in target/release/
```

## Components

| Component | Description |
|-----------|-------------|
| `zrc-agent` | Host agent - runs on machines you want to control |
| `zrc-desktop` | Desktop viewer - control remote machines |
| `zrc-relay` | Relay server - for NAT traversal |
| `zrc-rendezvous` | Signaling server - session initiation |
| `zrc-dirnode` | Directory node - device discovery |

## Quick Start

### 1. Start the Agent (on host machine)

```bash
./zrc-agent --foreground
```

### 2. Start the Desktop Viewer (on controller machine)

```bash
./zrc-desktop
```

### 3. Pair Devices

Use the pairing code displayed by the agent to connect from the desktop viewer.

## Next Steps

- [Configuration](configuration.md) - Customize your setup
- [Self-Hosting](self-hosting.md) - Run your own infrastructure
- [Security](security.md) - Understand the security model
