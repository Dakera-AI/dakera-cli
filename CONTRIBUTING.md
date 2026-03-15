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

Please use [GitHub Issues](https://github.com/dakera-ai/dakera-cli/issues) to report bugs or request features. Include:

- Steps to reproduce the issue
- Expected vs actual behavior
- `dk --version` output
- Operating system and Rust version

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
