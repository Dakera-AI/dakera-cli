[![Docs](https://img.shields.io/badge/docs-dakera.ai-D4A843)](https://dakera.ai/docs)
# ⚡ dakera-cli



[![CI](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml) [![Crate](https://img.shields.io/crates/v/dakera-cli?logo=rust)](https://crates.io/crates/dakera-cli) [![License: MIT](https://img.shields.io/github/license/Dakera-AI/dakera-cli)](LICENSE)
[![dakera.ai](https://img.shields.io/badge/dakera.ai-website-22c55e?style=flat-square)](https://dakera.ai) [![Docs](https://img.shields.io/badge/docs-dakera.ai%2Fdocs-3b82f6?style=flat-square)](https://dakera.ai/docs)
[![Docs](https://img.shields.io/badge/docs-dakera.ai-D4A843)](https://dakera.ai/docs)

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

## Quick Start

```bash
# Connect to a Dakera instance
dk init

# Store a memory
dk memory store my-agent "User prefers concise responses" --importance 0.8

# Recall memories
dk memory recall my-agent "user preferences" --top-k 5

# List namespaces
dk namespace list

# Check server health
dk health
```

## Connect to Dakera

```bash
# Set env vars (or use dk init for interactive setup)
export DAKERA_URL=http://your-server:3000
export DAKERA_API_KEY=your-key
```

## Global Flags

| Flag | Short | Default | Description |
|---|---|---|---|
| `--url` | `-u` | `http://localhost:3000` | Server URL (overrides config/env) |
| `--format` | `-f` | `table` | Output format: `table`, `json`, `compact` |
| `--verbose` | `-v` | false | Log HTTP requests and responses |
| `--profile` | `-p` | — | Named server profile from config |

```bash
# Machine-readable JSON output
dk --format json memory recall my-agent "recent tasks"

# Verbose mode — shows HTTP request/response timing
dk --verbose health

# Use a named profile
dk --profile staging namespace list
```

## Commands

| Command | Description |
|---|---|
| `dk health` | Check server health and connectivity |
| `dk init` | Interactive setup wizard |
| `dk namespace list\|policy` | Manage namespaces |
| `dk memory store\|recall\|get\|forget\|update\|importance\|consolidate\|feedback` | Agent memory operations |
| `dk session start\|end\|list\|memories` | Session lifecycle management |
| `dk agent list\|stats\|memories\|sessions` | Agent management |
| `dk vector upsert-one\|delete` | Vector store operations |
| `dk knowledge graph\|deduplicate` | Knowledge graph management |
| `dk keys list\|create\|delete\|usage` | API key management |
| `dk analytics overview\|latency\|throughput\|storage` | Platform analytics |
| `dk admin stats\|purge` | Admin operations |
| `dk ops metrics` | Operational metrics |
| `dk config` | Show or manage server profiles |
| `dk completion bash\|zsh\|fish\|powershell` | Shell completion |

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

## Documentation

→ [Full docs](https://dakera.ai/docs)  
→ [CLI reference](https://dakera.ai/docs/cli)

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
