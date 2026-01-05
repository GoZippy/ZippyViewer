# Security

ZRC is designed with security as a core principle, not an afterthought.

## Threat Model

ZRC protects against:

- **Man-in-the-middle attacks** on signaling and relay servers
- **Device impersonation** - cryptographic identity verification
- **Replay attacks** - nonces and timestamps on all messages
- **Downgrade attacks** - minimum cipher suite enforcement
- **Relay snooping** - end-to-end encryption (relay sees only ciphertext)
- **Unauthorized access** - capability-based permission tokens

## Cryptographic Primitives

| Purpose | Algorithm |
|---------|-----------|
| Identity keys | Ed25519 |
| Key exchange | X25519 |
| Symmetric encryption | ChaCha20-Poly1305 |
| Hashing | BLAKE3 / SHA-256 |
| KDF | HKDF-SHA256 |

## Key Management

### Device Identity

Each device generates a persistent Ed25519 keypair on first run. The public key serves as the device identity.

### Session Keys

Session keys are derived via X25519 key exchange with fresh ephemeral keys per session, providing forward secrecy.

### Pairing

Pairing uses a short-code challenge-response to establish mutual trust without requiring a trusted third party.

## Access Control

### Capability Tokens

Access is granted via time-limited capability tokens that specify:

- Allowed permissions (view, input, clipboard, file transfer)
- Validity window
- Rate limits

### Consent

- **Attended mode**: User must approve each session
- **Unattended mode**: Pre-authorized access with additional authentication

## Secure Updates

- All updates are signed with Ed25519
- Multiple signing keys supported (threshold signatures)
- Rollback protection via version monotonicity

## Best Practices

1. **Use unique pairing codes** - Don't reuse or share pairing codes
2. **Review permissions** - Grant only necessary capabilities
3. **Self-host for sensitive use** - Run your own infrastructure
4. **Keep software updated** - Security patches are released regularly
5. **Use firewall rules** - Restrict relay/rendezvous access as needed

## Reporting Security Issues

Please report security vulnerabilities privately via GitHub Security Advisories or email. Do not open public issues for security bugs.
