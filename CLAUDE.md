# dakera-cli

Rust CLI for the Dakera AI memory platform — manage memories, sessions, agents, namespaces,
keys, and vector search from the command line.

## Key Commands
```bash
cargo build --release        # Build CLI binary → target/release/dakera-cli
cargo test                   # Run tests
cargo clippy                 # Lint
cargo fmt                    # Format
./target/release/dakera-cli --help   # Show all subcommands
```

## Architecture
- `src/main.rs` — CLI entry; clap command routing
- `src/commands/` — One module per subcommand: memory, session, agent, keys, knowledge,
  vector, namespace, admin, health, ops, analytics, completion, init, config, index
- `src/config.rs` — Reads ~/.dakera/config.toml (API base URL + auth token)
- `src/output.rs` — Table / JSON / pretty-print output formatters
- `src/error.rs` — Error types wrapping API responses

## Conventions
- Commands map 1-to-1 to dakera REST API endpoints
- Default output: human-readable table; `--json` flag for machine-parseable output
- Config precedence: env vars > CLI flags > ~/.dakera/config.toml
- Releases published alongside the dakera server (same version tag)
