# dakera-cli

[![CI](https://github.com/dakera-ai/dakera-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/dakera-ai/dakera-cli/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/dakera-cli.svg)](https://crates.io/crates/dakera-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

Command-line interface for [Dakera](https://github.com/dakera-ai/dakera) -- manage your AI agent memory from the terminal.

## Installation

```bash
cargo install dakera-cli
```

Pre-built binaries for Linux, macOS (Intel and Apple Silicon), and Windows are available on the [Releases](https://github.com/dakera-ai/dakera-cli/releases) page.

## Usage

The CLI binary is called `dk`.

### Health Check

```bash
# Check server health
dk health

# Detailed health with diagnostics
dk health -d
```

### Namespaces

```bash
# List all namespaces
dk namespace list

# Get namespace details
dk namespace get my-namespace

# Create a namespace
dk namespace create my-namespace --dimension 384
```

### Vector Operations

```bash
# Query for similar vectors
dk vector query -n my-namespace -V 0.1,0.2,0.3 -k 5

# Upsert a single vector
dk vector upsert-one -n my-namespace -i vec1 -V 0.1,0.2,0.3

# Bulk upsert from JSON file
dk vector upsert -n my-namespace -f vectors.json

# Delete vectors by ID
dk vector delete -n my-namespace -i id1,id2,id3

# Export vectors
dk vector export -n my-namespace --include-vectors
```

### Agent Management

```bash
# List all agents
dk agent list

# View agent statistics
dk agent stats my-agent

# View agent memories
dk agent memories my-agent --type semantic

# View agent sessions
dk agent sessions my-agent --active-only
```

### Memory Operations

```bash
# Store a memory
dk memory store my-agent "The user prefers dark mode" -t semantic -i 0.8

# Recall memories by semantic search
dk memory recall my-agent "user preferences" -k 5

# Get a specific memory
dk memory get my-agent memory-id-123

# Consolidate similar memories
dk memory consolidate my-agent --threshold 0.8 --dry-run

# Submit feedback on a memory
dk memory feedback my-agent memory-id-123 "highly relevant" -s 0.9
```

### Session Management

```bash
# Start a new session
dk session start my-agent

# End a session
dk session end session-id-123 -s "Completed onboarding flow"

# List sessions
dk session list -a my-agent --active-only

# View session memories
dk session memories session-id-123
```

### Knowledge Graph

```bash
# Build knowledge graph from a seed memory
dk knowledge graph my-agent -m memory-id -d 3

# Full knowledge graph for an agent
dk knowledge full-graph my-agent --max-nodes 100

# Deduplicate memories
dk knowledge deduplicate my-agent --threshold 0.9 --dry-run
```

### Analytics

```bash
# Platform overview
dk analytics overview -p 24h

# Latency statistics
dk analytics latency -p 1h

# Storage breakdown
dk analytics storage
```

### Administration

```bash
# Server diagnostics
dk ops diagnostics

# Background jobs
dk ops jobs

# Trigger compaction
dk ops compact -n my-namespace

# API key management
dk keys create my-key -p admin -e 90
dk keys list
dk keys rotate key-id-123

# Cluster status
dk admin cluster-status

# Backup management
dk admin backup-create
dk admin backup-list
```

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `DAKERA_URL` | Server URL | `http://localhost:3000` |
| `DAKERA_NAMESPACE` | Default namespace | `default` |

```bash
# Connect to a remote server
export DAKERA_URL=https://my-dakera-server.example.com

# Or pass the URL directly
dk -u https://my-dakera-server.example.com health
```

## Output Formats

```bash
# Table format (default)
dk namespace list

# JSON output
dk namespace list -f json

# Compact JSON (single line)
dk namespace list -f compact
```

## Related Repositories

| Repository | Description |
|------------|-------------|
| [dakera](https://github.com/dakera-ai/dakera) | Core AI agent memory engine (Rust) |
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | TypeScript/JavaScript SDK |
| [dakera-go](https://github.com/dakera-ai/dakera-go) | Go SDK |
| [dakera-rs](https://github.com/dakera-ai/dakera-rs) | Rust SDK |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP Server for AI agent memory |
| [dakera-dashboard](https://github.com/dakera-ai/dakera-dashboard) | Admin dashboard (Leptos/WASM) |
| [dakera-docs](https://github.com/dakera-ai/dakera-docs) | Documentation and API reference |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Deployment configs and Docker Compose |
| [dakera-cortex](https://github.com/dakera-ai/dakera-cortex) | Flagship demo with AI agents |

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
