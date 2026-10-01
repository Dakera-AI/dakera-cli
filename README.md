# ⚡ dakera-cli

[![CI](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/Dakera-AI/dakera-cli/actions/workflows/ci.yml) [![Crate](https://img.shields.io/crates/v/dakera-cli?logo=rust)](https://crates.io/crates/dakera-cli) [![Downloads](https://img.shields.io/crates/d/dakera-cli)](https://crates.io/crates/dakera-cli) [![License: MIT](https://img.shields.io/github/license/Dakera-AI/dakera-cli)](LICENSE) [![LoCoMo 88.2%](https://img.shields.io/badge/LoCoMo-88.2%25-22c55e?style=flat-square)](https://dakera.ai/benchmark) [![Docs](https://img.shields.io/badge/docs-dakera.ai%2Fdocs-3b82f6?style=flat-square)](https://dakera.ai/docs) [![dakera.ai](https://img.shields.io/badge/dakera.ai-website-22c55e?style=flat-square)](https://dakera.ai) [![Playground](https://img.shields.io/badge/playground-try%20it-ff6b35?style=flat-square)](https://dakera.ai/playground)

Command-line interface for [Dakera AI](https://dakera.ai) — inspect and manage a Dakera memory instance from the terminal.

> The Dakera memory engine scores **88.2% Recall@20 on LoCoMo** (1,536 evaluated questions · LLM-judged retrieval recall) — [benchmark details](https://dakera.ai/benchmark)

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

### Homebrew (macOS / Linux)

```bash
brew install dakera-ai/tap/dk
```

### APT (Debian / Ubuntu)

```bash
curl -fsSL https://dakera-ai.github.io/apt-repo/KEY.gpg \
  | sudo gpg --dearmor -o /usr/share/keyrings/dakera-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/dakera-archive-keyring.gpg] https://dakera-ai.github.io/apt-repo stable main" \
  | sudo tee /etc/apt/sources.list.d/dakera.list
sudo apt-get update && sudo apt-get install -y dk
```

### YUM / DNF (Fedora / RHEL / CentOS)

```bash
sudo tee /etc/yum.repos.d/dakera.repo << 'EOF'
[dakera]
name=Dakera AI
baseurl=https://dakera-ai.github.io/rpm-repo/
enabled=1
gpgcheck=0
EOF
sudo dnf install -y dk
```

### Cargo

```bash
# From crates.io (compiles from source)
cargo install dakera-cli

# Pre-built binary via cargo-binstall (faster)
cargo binstall dakera-cli
```

### Binary download

Pre-built binaries for macOS (arm64/x64), Linux (x64/arm64), and Windows are available on the [releases page](https://github.com/Dakera-AI/dakera-cli/releases).

| Platform | File |
|---|---|
| macOS (Apple Silicon) | `dk-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `dk-x86_64-apple-darwin.tar.gz` |
| Linux x64 | `dk-x86_64-unknown-linux-musl.tar.gz` |
| Linux arm64 | `dk-aarch64-unknown-linux-musl.tar.gz` |
| Windows x64 | `dk-x86_64-pc-windows-msvc.zip` |

---

## Quick Start

```bash
# 1. Configure the CLI (server URL + API key)
dk init

# 2. Verify connectivity
dk health

# 3. Store your first memory
dk memory store my-agent "The user prefers concise responses" --importance 0.8

# 4. Recall by semantic query
dk memory recall my-agent "user preferences" --top-k 5

# 5. Full-text BM25 search
dk text search "user preferences" --namespace default
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
dk config profile add staging --url http://staging:3000 --key dk-staging-key
dk config profile use staging
dk --profile staging namespace list
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
dk --format json memory recall my-agent "recent tasks"
dk --format compact namespace list | jq '.[].name'
dk --verbose memory store my-agent "new memory"
```

---

## Commands

### `dk health`

Check server health and connectivity. Reads the server's JSON directly, so a
v0.12 server that is still loading its models (`/health` answers `503` with a
`Retry-After` header) is reported as such, with exit code 6 — never as healthy.

```bash
dk health                  # status; lists degraded components, config warnings, embed migration
dk health --detailed       # + live / ready probes, version, build, uptime, diagnostics
dk health ready            # GET /health/ready  (exit 6 and the reason/downloads while starting)
dk health live             # GET /health/live
```

---

### `dk capabilities`

What the connected server supports (`GET /v1/capabilities`, Dakera v0.12+): the
active embedding model, search mode, scoring strategy, query languages (`lang`)
and which opt-in features are on (attachments, speech to text, image indexing,
records). A v0.11 server has no such route: the command exits 3 and says so.

```bash
dk capabilities
dk --format json capabilities
```

---

### `dk attachment`

Files that agent memories can point at (Dakera v0.12, **opt-in**: the server
answers `501 FEATURE_DISABLED` until `DAKERA_ATTACHMENTS` is set; image indexing
also needs `DAKERA_VISION`). `dk capabilities` shows what is on.

```bash
dk attachment upload _dakera_agent_my-agent note.wav    # media type guessed from the extension
dk attachment list _dakera_agent_my-agent
dk attachment download _dakera_agent_my-agent sha256:<hex> -o note.wav
dk attachment delete _dakera_agent_my-agent sha256:<hex>   # 409 while a memory references it

# Speech to text: a WAV attachment becomes a memory (a background job)
dk attachment transcribe uploads sha256:<hex> --agent-id my-agent --tag voice --wait
# Visual memory from a PNG (needs DAKERA_VISION)
dk attachment index uploads sha256:<hex> --agent-id my-agent --content "page 3" --wait
dk attachment job uploads sha256:<hex> job_1a2b3c4d_0 --kind transcribe
```

A memory can only reference an attachment stored in its own namespace,
`_dakera_agent_<agent_id>`; `transcribe` and `index` copy an attachment from any
namespace you can read into the agent's. `--wait` polls the job (progress goes
to stderr) and exits 5 if the job fails with a `400`, 6 on a `5xx`.

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
dk memory recall my-agent "UI preferences" --top-k 10

# Full-text search within an agent's memories
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
# Search one namespace (the server has no cross-namespace full-text route)
dk text search "machine learning" --namespace default

# Search within a specific namespace
dk text search "temporal reasoning" --namespace my-ns --limit 20
```

---

### `dk session`

Manage agent sessions.

```bash
dk session start my-agent
dk session end sess-abc123
dk session list --agent-id my-agent --active-only
dk session get sess-abc123
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
# Build a knowledge graph from a memory
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

### `dk admin`

Cluster administration, caching, backups, and server configuration.

```bash
# Cluster overview
dk admin cluster-status
dk admin cluster-nodes

# Namespace index management
dk admin optimize my-ns
dk admin index-stats my-ns
dk admin rebuild-indexes my-ns

# Cache management
dk admin cache-stats
dk admin cache-clear
dk admin cache-clear --namespace my-ns

# Server configuration
dk admin config-get
dk admin config-set --key max_connections --value 100

# Namespace quotas (a hard quota is enforced since v0.12: writes over it get 413)
dk admin quotas-get
dk admin quotas-set --data '{"max_vectors": 100000, "enforcement": "hard"}'              # default quota
dk admin quotas-set -n my-ns --data '{"max_vectors": 10000, "enforcement": "soft"}'      # one namespace

# Performance diagnostics
dk admin slow-queries --limit 10

# After an upgrade to v0.12: the one-time background re-embed (also in `dk health`)
dk admin embed-migration

# Encryption at rest (v0.12 keyring): status, rotation per namespace, re-seal
dk admin encryption-status
dk admin encryption-rotate -n my-ns                      # a new random key for one namespace
dk admin encryption-rotate --new-key-env MY_NEW_KEY      # global, from a passphrase in the environment
dk admin encryption-reseal --wait-secs 30

# Backups
dk admin backup-create --name nightly --type full --wait
dk admin backup-list
dk admin backup-get <backup-id>
dk admin backup-schedule                                 # an enabled schedule runs since v0.12
dk admin backup-download <backup-id> -o nightly.json.gz  # super_admin
dk admin backup-upload nightly.json.gz                   # super_admin
dk admin backup-restore <backup-id> -n my-ns --wait      # super_admin; add --overwrite --yes for a point-in-time restore
dk admin backup-restore-status <restore-id>
dk admin backup-delete <backup-id>
```

All of these need a **global** key. A key pinned to namespaces gets `403` on
node-wide routes even with the `admin` scope, and backup download, upload and
restore need `super_admin` (see [Permissions](#permissions-v012)). The previous
`backup-create` sent no `name`, which the server requires, and `quotas-set`
called a route that does not exist; both now work.

---

### `dk config`

Show or manage connection profiles.

```bash
dk config
dk config profile add staging --url http://staging:3000
dk config profile use staging
dk config profile list
```

---

### `dk completion`

Generate shell completion scripts.

```bash
dk completion bash --install
dk completion zsh --install
dk completion fish --install
dk completion powershell
```

---

## Dakera v0.12 and compatibility

`dk` 0.8 works against Dakera **v0.11.108 and v0.12.0** servers. The new
commands call the server's REST API directly (no SDK change), so they need the
server release that has the route:

| Command | v0.11.108 | v0.12.0 |
|---|---|---|
| everything in 0.7 (`memory`, `namespace`, `session`, `keys`, ...) | yes | yes |
| `dk health`, `dk health ready`, `dk health live` | yes | yes (+ `degraded`, `config_warnings`, `embed_migration`) |
| `dk capabilities` | exits 3 (no route) | yes |
| `dk attachment ...` | no | yes, once `DAKERA_ATTACHMENTS` is on |
| `dk admin embed-migration`, `encryption-status`, `encryption-reseal` | no | yes |
| other `dk admin` commands (`encryption-rotate`, `backup-*`, `quotas-*`, ...) | routes that v0.11.108 already had | same routes; v0.12 changes who may call them and what they do (below) |

### Permissions (v0.12)

* **Keys pinned to namespaces get `403`** on node-wide routes (`/admin/*`:
  backups, encryption, quotas, config, cluster), even with the `admin` scope.
* **Backup download, upload and restore need a global `super_admin` key**: a
  backup bundle carries every API key hash, so an `admin` key can no longer take
  one (or restore an edited one).

`dk` turns those answers into a message that says what to do, for example:

```
✗ Permission denied: Request failed (403 Forbidden) [INSUFFICIENT_SCOPE]: Insufficient scope for this operation (required: super_admin, actual: admin)
  hint: this needs a global super_admin key: since v0.12 backup download, upload and restore are refused to admin keys ...
```

### Errors

v0.12 answers every error with a JSON body (`{"error", "code", "status", "details"}`)
and every `503` carries `Retry-After`. `dk` keeps those fields: with
`--format json` the error on stderr gains `http_status`, `server_code`, `details`
and `retry_after_secs`. A `413` (a body over a size limit such as
`DAKERA_ATTACHMENT_MAX_BYTES`, or a hard namespace quota) exits 5; a `501` (a
feature that is switched off, `FEATURE_DISABLED`, or a configuration the server
cannot serve) and a `503` exit 6. Retry a `503` after the `Retry-After` seconds.

### Server-side commands (not `dk` commands)

Two operator commands ship in the **server binary**, not in `dk`; run them with
the server's image, environment and volumes:

```bash
# Check a configuration with the new image before upgrading: exits 0 (the server
# would start, warnings included) or 78 (it would refuse). Starts nothing.
docker run --rm <same env and volumes> <v0.12 image> --check-config

# Go back to v0.11.108: run once, after the v0.12 server has stopped. Prints a JSON
# report; exit 0 = the data is v0.11.108's, 1 = something still needs fixing (run it
# again), 78 = it refused and changed nothing.
docker run --rm <same env and volumes> <v0.12 image> downgrade
```

See the server's `docs/v0.12/UPGRADE.md` ("Going back to v0.11.108") for what
`downgrade` converts and when it refuses.

---

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Connection error (server unreachable) |
| 3 | Not found |
| 4 | Permission denied / authentication failure (`401`, `403`) |
| 5 | Invalid input (`400`, `409`, `413`, `415`, `422`, or a bad argument) |
| 6 | Server-side error (5xx, including `501` feature disabled and `503` busy/starting) |

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

<sub>Part of the Dakera AI open-core ecosystem. Built with Rust. Self-hosted. Zero dependencies.</sub>
