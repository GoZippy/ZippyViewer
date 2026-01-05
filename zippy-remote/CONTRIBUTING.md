# Contributing to ZRC

Thank you for your interest in contributing to ZRC! This document provides guidelines and information for contributors.

## Code of Conduct

Please be respectful and constructive in all interactions. We want ZRC to be a welcoming project for everyone.

## Getting Started

### Prerequisites

- Rust 1.75+ (stable toolchain)
- Git
- Platform-specific development tools (see below)

### Platform-Specific Dependencies

#### Windows
- Visual Studio Build Tools 2019 or later
- Windows SDK

#### macOS
- Xcode Command Line Tools
- macOS 12.0+ SDK

#### Linux
- GCC or Clang
- pkg-config
- libssl-dev (or equivalent)

### Setting Up Your Development Environment

```bash
# Clone the repository
git clone https://github.com/yourusername/ZippyViewer.git
cd ZippyViewer/zippy-remote

# Install Rust toolchain
rustup toolchain install stable
rustup component add rustfmt clippy

# Build and test
cargo build
cargo test
```

## Development Workflow

### Branching Strategy

- `main` - Stable release branch
- `develop` - Integration branch for upcoming releases
- `feature/*` - Feature branches
- `fix/*` - Bug fix branches
- `security/*` - Security-related changes

### Making Changes

1. **Create a branch**: `git checkout -b feature/my-feature develop`
2. **Make your changes**: Follow the coding standards below
3. **Test your changes**: `cargo test`
4. **Format your code**: `cargo fmt`
5. **Check for issues**: `cargo clippy`
6. **Commit your changes**: Use conventional commit messages
7. **Push and create a PR**: Target the `develop` branch

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```
feat(relay): add bandwidth throttling per allocation
fix(crypto): handle edge case in envelope decryption
docs(readme): add deployment instructions
```

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` with default settings
- Fix all `cargo clippy` warnings
- Document public APIs with doc comments

### Error Handling

- Use `thiserror` for error types
- Use `anyhow` for application-level errors
- Provide context in error messages

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("failed to connect to {address}: {source}")]
    ConnectionFailed {
        address: String,
        #[source]
        source: std::io::Error,
    },
}
```

### Testing

- Write unit tests for all public functions
- Write integration tests for workflows
- Use property-based testing for cryptographic code
- Target 80%+ code coverage

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert!(result.is_ok());
    }
}
```

### Security Considerations

When working on security-sensitive code:

1. **Never log sensitive data** (keys, passwords, session tokens)
2. **Use constant-time comparisons** for cryptographic values
3. **Zeroize secrets** when they're no longer needed
4. **Validate all inputs** at trust boundaries
5. **Consider timing attacks** in crypto code

## Pull Request Process

### Before Submitting

- [ ] All tests pass locally
- [ ] Code is formatted with `cargo fmt`
- [ ] No clippy warnings
- [ ] Documentation updated if needed
- [ ] CHANGELOG updated if applicable

### PR Description

Provide a clear description including:
- What changes were made
- Why the changes were made
- How to test the changes
- Any breaking changes

### Review Process

1. Automated CI checks must pass
2. At least one maintainer review required
3. Security-sensitive changes require security team review
4. Changes may require updates based on feedback

## Security Vulnerabilities

If you discover a security vulnerability, please do NOT open a public issue. Instead:

1. Email security@example.com with details
2. Allow up to 48 hours for initial response
3. Work with us to develop a fix
4. We'll coordinate disclosure timing

## Areas for Contribution

### Good First Issues

Look for issues labeled `good-first-issue` for beginner-friendly tasks.

### Help Wanted

Issues labeled `help-wanted` are features we'd love community help with.

### Current Priorities

1. **Platform Support**: Improving Linux Wayland support
2. **Performance**: Optimizing video encoding/decoding
3. **Testing**: Expanding test coverage
4. **Documentation**: API docs and tutorials

## Questions?

- Open a Discussion on GitHub
- Join our community chat (link TBD)
- Email the maintainers

Thank you for contributing to ZRC!
