[![Docs](https://img.shields.io/badge/docs-dakera.ai-D4A843)](https://dakera.ai/docs)
# ⚡ dakera-cli

[![CI](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml) [![Crate](https://img.shields.io/crates/v/dakera-cli?logo=rust)](https://crates.io/crates/dakera-cli) [![License: MIT](https://img.shields.io/github/license/Dakera-AI/dakera-cli)](LICENSE)
[![dakera.ai](https://img.shields.io/badge/dakera.ai-website-22c55e?style=flat-square)](https://dakera.ai) [![Docs](https://img.shields.io/badge/docs-dakera.ai%2Fdocs-3b82f6?style=flat-square)](https://dakera.ai/docs)

Command-line interface for Dakera AI — inspect and manage a Dakera instance from the terminal.

Part of [Dakera AI](https://dakera.ai) — the memory engine for AI agents.

> The Dakera memory engine scores **87.6% on LoCoMo** (1,540 questions, standard eval) — [benchmark details](https://dakera.ai/benchmark)

---

## Run Dakera

You need a running Dakera server to connect to. The fastest way:

```bash
docker run -d \
  --name dakera \
  -p 3000:3000 \
  -e DAKERA_ROOT_API_KEY=dk-mykey \
  ghcr.io/dakera-ai/dakera:latest
```

For persistent storage (recommended):

```bash
curl -sSfL https://raw.githubusercontent.com/Dakera-AI/dakera-deploy/main/docker-compose.yml \
  -o docker-compose.yml
DAKERA_ROOT_API_KEY=dk-mykey docker compose up -d

curl http://localhost:3000/health  # → {"status":"ok"}
```

Full deployment guide (Docker Compose, Kubernetes, Helm): [dakera-deploy](https://github.com/Dakera-AI/dakera-deploy)

---

## Install

```bash
cargo install dakera-cli
```

Or download a pre-built binary from the [releases page](https://github.com/Dakera-AI/dakera-cli/releases).

---

## Quick Start

```bash
# Interactive setup (sets server URL + API key)
dk init

# Check server health
dk health

# Store a memory for an agent
dk memory store my-agent "User prefers concise responses" --importance 0.8

# Recall memories by semantic query
dk memory recall my-agent "user preferences" --top-k 5

# Full-text BM25 search
dk text search "user preferences" --namespace default

# List namespaces
dk namespace list
```

---

## Configuration

### Environment variables

| Variable | Description | Default |
|---|---|---|
| `DAKERA_URL` | Server base URL | `http://localhost:3000` |
| `DAKERA_API_KEY` | API key for authentication | — |
| `DAKERA_PROFILE` | Named profile to use | active profile in config |

### Config file

`dk init` creates `~/.dakera/config.toml`:

```toml
[server]
url = "http://localhost:3000"
api_key = "dk-mykey"

[defaults]
namespace = "default"
```

### Named profiles

```bash
# Add a profile
dk config profile add staging --url http://staging:3000 --key dk-staging-key

# Use a profile for one command
dk --profile staging namespace list

# Set a profile as active
dk config profile use staging
```

### Precedence

Environment variables > CLI flags > config file > defaults.

---

## Global Flags

| Flag | Short | Default | Description |
|---|---|---|---|
| `--url` | `-u` | `http://localhost:3000` | Server URL |
| `--format` | `-f` | `table` | Output format: `table`, `json`, `compact` |
| `--verbose` | `-v` | false | Log HTTP requests and response timing |
| `--profile` | `-p` | — | Named server profile |

```bash
# Machine-readable JSON output
dk --format json memory recall my-agent "recent tasks"

# Compact single-line JSON (for piping/scripting)
dk --format compact namespace list | jq '.[].name'

# Show HTTP request/response timing
dk --verbose memory store my-agent "new memory"
```

---

## Commands

### `dk health`

Check server health and connectivity.

```bash
dk health
dk health --detailed
```

---

### `dk namespace`

Manage namespaces.

```bash
dk namespace list
dk namespace create my-ns
dk namespace policy --namespace my-ns
```

---

### `dk memory`

Store, recall, search, and manage agent memories. This is the primary interface to Dakera.

```bash
# Store a memory
dk memory store my-agent "The user likes dark mode" --importance 0.8 --type semantic

# Recall by semantic query
dk memory recall my-agent "UI preferences" --top-k 10 --type semantic

# Search with full-text filters
dk memory search my-agent "dark mode" --top-k 5

# Get a specific memory by ID
dk memory get my-agent mem-abc123

# Update a memory
dk memory update my-agent mem-abc123 --content "Updated content"

# Delete a single memory
dk memory forget my-agent mem-abc123

# Batch delete by filters (dry-run first!)
dk memory batch-forget my-agent --min-importance 0.3 --dry-run
dk memory batch-forget my-agent --min-importance 0.3 --max-age-days 90

# Update importance scores
dk memory importance my-agent --ids mem-1,mem-2 --value 0.9

# Consolidate similar memories into summaries
dk memory consolidate my-agent --dry-run

# Submit recall quality feedback
dk memory feedback my-agent mem-abc123 "Highly relevant" --score 1.0
```

---

### `dk text`

Full-text (BM25) search across memories.

```bash
# Search all namespaces
dk text search "machine learning"

# Search within a specific namespace
dk text search "temporal reasoning" --namespace my-ns --limit 20
```

---

### `dk session`

Manage agent sessions.

```bash
# Start a session
dk session start my-agent

# End a session
dk session end sess-abc123

# List sessions (optionally filter to active only)
dk session list --agent-id my-agent --active-only

# Get session details
dk session get sess-abc123

# List memories stored during a session
dk session memories sess-abc123
```

---

### `dk agent`

View and manage agents.

```bash
dk agent list
dk agent stats my-agent
dk agent memories my-agent --type episodic --limit 20
dk agent sessions my-agent --active-only
```

---

### `dk knowledge`

Knowledge graph management and memory summarization.

```bash
# Build a knowledge graph from a specific memory
dk knowledge graph my-agent --memory-id mem-abc123 --depth 3

# Full knowledge graph for an agent
dk knowledge full-graph my-agent --max-nodes 100

# Summarize a set of memories into a new memory
dk knowledge summarize my-agent --memory-ids m1,m2,m3 --dry-run

# Find and remove duplicate memories
dk knowledge deduplicate my-agent --threshold 0.9 --dry-run
```

---

### `dk index`

Index management.

```bash
dk index stats --namespace my-ns
dk index fulltext-stats --namespace my-ns
dk index rebuild --namespace my-ns --dry-run
dk index rebuild --namespace my-ns --index-type vector --yes
```

---

### `dk keys`

API key management.

```bash
dk keys list
dk keys create my-key --permissions read,write
dk keys delete key-abc123
dk keys usage key-abc123
```

---

### `dk analytics`

Platform analytics and metrics.

```bash
dk analytics overview --period 24h
dk analytics latency --period 7d
dk analytics throughput --period 1h
dk analytics storage
```

---

### `dk ops`

Operations and maintenance.

```bash
dk ops stats
dk ops diagnostics
dk ops jobs
dk ops compact --namespace my-ns
```

---

### `dk config`

Show or manage server configuration and profiles.

```bash
dk config
dk config profile add staging --url http://staging:3000
dk config profile use staging
dk config profile list
```

---

### `dk completion`

Shell completion scripts.

```bash
# Bash
dk completion bash --install

# Zsh
dk completion zsh --install

# Fish
dk completion fish --install

# PowerShell
dk completion powershell
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Connection error (server unreachable) |
| 3 | Not found |
| 4 | Permission denied / authentication failure |
| 5 | Invalid input |
| 6 | Server-side error (5xx) |

Scripts can check `$?` after each command.

---

## Related

| Repo | What it is |
|---|---|
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | TypeScript SDK |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server · 14 core tools (86+ via profiles) |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Self-host Dakera |

---

**[dakera.ai](https://dakera.ai)** · [Documentation](https://dakera.ai/docs) · [Request Early Access](https://dakera.ai#cta)

<sub>Part of the Dakera AI open-source ecosystem. Built with Rust. Self-hosted. Zero dependencies.</sub>
