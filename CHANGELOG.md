# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.1] - 2026-03-24

### CI

- Add `deploy-binary` job to release workflow — attaches compiled binaries as release assets (INFRA-1)
- SHA-pin `webfactory/ssh-agent` in CI and release workflows — supply chain security hardening

### Changed

- Reposition product messaging as AI agent memory platform (DAK-729)

## [0.3.2] - 2026-03-21

### Changed

- Bumped `dakera-client` dependency from `0.2.0` → `0.6` to track the current SDK.
  No functional changes — all existing CLI operations are compatible. Picks up
  improvements from SDK v0.3.0–v0.6.1 (typed `EmbeddingModel`, `ServerErrorCode`,
  `configure_namespace`, SSE events, cross-agent network types).

## [0.3.1] - 2026-03-20

### Fixed

- `ConfigFile` now implements `Default` using `default_profile_name()` — fixes profile name inconsistency on fresh installs
- Rustfmt formatting fixes

### Added

- Unit tests for config and output modules (DAK-173)

### Chore

- Upgrade GitHub Actions runners to Node.js 24 compatible versions

## [0.3.0] - 2026-03-19

### Added

- `dk init` onboarding wizard with file-based config (DX-1)
- `dk completion bash|zsh|fish [--install]` — shell completion generation and auto-install (DX-2)
- Profile management: `dk profile list|create|switch|delete|show` (DX-3)

### Fixed

- zsh completion format-string brace escaping
- Cleaned up `&'static str` returns

### Security

- Add explicit `GITHUB_TOKEN` permissions to CI workflow

## [0.2.0] - 2025-03-15

### Added

- Initial release as standalone CLI tool (extracted from [dakera](https://github.com/dakera-ai/dakera) monorepo)
- **Health**: `dk health` with detailed diagnostics (`-d`)
- **Namespaces**: list, get, create, delete
- **Vectors**: upsert (batch and single), query, query-file, delete, multi-search, unified-query, aggregate, export, explain, upsert-columns
- **Agents**: list, memories, stats, sessions
- **Memory**: store, recall, get, update, forget, search, importance, consolidate, feedback
- **Sessions**: start, end, get, list, memories
- **Knowledge**: graph, full-graph, summarize, deduplicate
- **Analytics**: overview, latency, throughput, storage
- **Admin**: cluster-status, cluster-nodes, optimize, index-stats, rebuild-indexes, cache-stats, cache-clear, config-get, config-set, quotas, slow-queries, backup-create, backup-list, backup-restore, backup-delete, configure-ttl
- **Keys**: create, list, get, delete, deactivate, rotate, usage
- **Ops**: diagnostics, jobs, job details, compact, shutdown, metrics
- Output format support: table, JSON, compact JSON
- Configuration via `DAKERA_URL` and `DAKERA_NAMESPACE` environment variables
- Cross-platform binary releases (Linux x86_64, macOS x86_64/aarch64, Windows x86_64)
