# ⚡ dakera-cli

[![CI](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml) [![Crate](https://img.shields.io/crates/v/dakera-cli?logo=rust)](https://crates.io/crates/dakera-cli) [![License: MIT](https://img.shields.io/github/license/Dakera-AI/dakera-cli)](LICENSE)

Command-line interface for Dakera AI — inspect and manage a Dakera instance from the terminal.

Part of [Dakera AI](https://dakera.ai) — the memory engine for AI agents.

> The Dakera memory engine scores **87.6% on LoCoMo** (1,540 questions, standard eval) — [benchmark details](https://dakera.ai/benchmark)

---

## Run Dakera

You need a running Dakera server to connect to. The fastest way:

```bash
docker run -d \
  --name dakera \
  -p 3300:3300 \
  -e DAKERA_ROOT_API_KEY=dk-mykey \
  ghcr.io/dakera-ai/dakera:latest
```

For persistent storage (recommended):

```bash
curl -sSfL https://raw.githubusercontent.com/Dakera-AI/dakera-deploy/main/docker-compose.yml \
  -o docker-compose.yml
DAKERA_API_KEY=dk-mykey docker compose up -d

curl http://localhost:3300/health  # → {"status":"ok"}
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
dk memories store \
  --agent my-agent \
  --content "User prefers concise responses" \
  --importance 0.8

# Query memories
dk memories search \
  --agent my-agent \
  --query "user preferences" \
  --top-k 5
```

## Connect to Dakera

```bash
# Set env vars (or use dk init for interactive setup)
export DAKERA_URL=http://your-server:3300
export DAKERA_API_KEY=your-key
```

## Documentation

→ [Full docs](https://dakera.ai/docs)  
→ [CLI reference](https://dakera.ai/docs/cli)

## Related

| Repo | What it is |
|---|---|
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | TypeScript SDK |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server · 83 tools |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Self-host Dakera |

---

*Part of the Dakera AI open core. The engine is proprietary. The tools are yours.*
