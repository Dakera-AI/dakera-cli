# Contributing to dakera-cli

Thank you for your interest in contributing to dakera-cli.

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) 1.75 or later (stable toolchain)
- A running Dakera server for integration testing

### Building

```bash
git clone https://github.com/dakera-ai/dakera-cli.git
cd dakera-cli
cargo build
```

### Running

```bash
# Run the CLI directly
cargo run -- health

# Or install locally
cargo install --path .
dk health
```

### Testing

```bash
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check compilation
cargo check
```

## Pull Request Process

1. Fork the repository and create a feature branch from `main`.
2. Ensure your code compiles without warnings (`cargo clippy -- -D warnings`).
3. Format your code with `cargo fmt`.
4. Add or update tests as appropriate.
5. Update `CHANGELOG.md` with a description of your changes.
6. Open a pull request with a clear description of your changes.

## Reporting Issues

Use the [Bug Report](https://github.com/Dakera-AI/dakera-cli/issues/new?template=bug_report.md) template to report bugs. Please include:
- Steps to reproduce the issue
- Expected vs actual behavior
- `dk --version` output, operating system, and Rust version

Have a feature idea? Use the [Feature Request](https://github.com/Dakera-AI/dakera-cli/issues/new?template=feature_request.md) template.

## Security Vulnerabilities

**Do not open public issues for security vulnerabilities.** See [SECURITY.md](.github/SECURITY.md) for responsible disclosure instructions — report via [GitHub Security Advisories](https://github.com/dakera-ai/dakera-cli/security/advisories/new).

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
