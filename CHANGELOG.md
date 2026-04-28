# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.5] - 2026-04-28

### Fixed

- Bumped `dakera-client` from 0.9 to 0.11 with API adaptation for
  `SessionEndResponse` field changes in dakera server v0.11.41.
  ([#48](https://github.com/Dakera-AI/dakera-cli/pull/48))

### Dependencies

- Bumped `tokio` from 1.37 to 1.52.
  ([#49](https://github.com/Dakera-AI/dakera-cli/pull/49))
- Bumped `thiserror` from 1.0.69 to 2.0.18.
  ([#45](https://github.com/Dakera-AI/dakera-cli/pull/45))
- Bumped `dirs` from 5.0.1 to 6.0.0.
  ([#47](https://github.com/Dakera-AI/dakera-cli/pull/47))
- Bumped `httpmock` from 0.7.0 to 0.8.3 (dev).
  ([#40](https://github.com/Dakera-AI/dakera-cli/pull/40))
- Bumped `toml` from 0.8.23 to 1.1.2.
  ([#41](https://github.com/Dakera-AI/dakera-cli/pull/41))
- Bumped `assert_cmd` from 2.2.0 to 2.2.1 (dev).
  ([#38](https://github.com/Dakera-AI/dakera-cli/pull/38))
- Bumped `clap` from 4.6.0 to 4.6.1.
  ([#42](https://github.com/Dakera-AI/dakera-cli/pull/42))
- Bumped `rustls-webpki` from 0.103.12 to 0.103.13.
  ([#35](https://github.com/Dakera-AI/dakera-cli/pull/35))

### CI

- Pinned Rust toolchain to 1.95.0 for deterministic builds.
  ([#44](https://github.com/Dakera-AI/dakera-cli/pull/44))
- Bumped `appleboy/ssh-action` from 1.0.3 to 1.2.5.
  ([#36](https://github.com/Dakera-AI/dakera-cli/pull/36))
- Bumped `softprops/action-gh-release` from 2 to 3.
  ([#37](https://github.com/Dakera-AI/dakera-cli/pull/37))
- Added manual `workflow_dispatch` trigger for crates.io publish.
- Fixed `cargo publish --allow-dirty` for CI publishing.
  ([#34](https://github.com/Dakera-AI/dakera-cli/pull/34))

## [0.5.4] - 2026-04-17

### CI

- Remove obsolete SSH agent setup from all CI jobs.
  ([#30](https://github.com/Dakera-AI/dakera-cli/pull/30))

### Dependencies

- Bumped `rand` from 0.9.2 to 0.9.4.
  ([#29](https://github.com/Dakera-AI/dakera-cli/pull/29))
- **Security — rustls-webpki CVE patch**: Updated to `rustls-webpki 0.103.12` addressing
  GHSA-xgp8-3hg3-c2mh and GHSA-965h-392x-2mh5 (CVSS 2.2 LOW).
  ([#32](https://github.com/Dakera-AI/dakera-cli/pull/32))

## [0.5.3] - 2026-04-13

### Added

- Integration test harness: 7 tests covering health, namespace list, and namespace policy
  get/set using `httpmock` + `assert_cmd` (DAK-1492).

### CI

- Add `cargo-audit` CVE scanning to CI pipeline — runs on every push and PR (#26).
- Skip `cargo-audit` installation when binary already exists on self-hosted runner (#27).

### Changed

- Updated README to reflect open-core product model and current platform positioning (#28).

## [0.5.2] - 2026-04-01

### Added

- `dk namespace policy get <namespace>` — display the full memory lifecycle policy for a
  namespace: differential TTLs, decay curves, spaced repetition settings, COG-3 background
  consolidation config, and SEC-5 per-namespace rate limits.
- `dk namespace policy set <namespace> [flags]` — patch any subset of policy fields without
  touching the rest. Fetches the current policy first, applies only the flags supplied, clears the
  read-only `consolidated_count` field, then PUTs the result. All fields from COG-1, COG-3, and
  SEC-5 are exposed as flags (see `--help` for the full list).
- Bumps `dakera-client 0.8 → 0.9` to access `get_memory_policy`, `set_memory_policy`, and the
  updated `MemoryPolicy` struct with SEC-5 rate-limiting fields (CLI-2).

## [0.5.1] - 2026-03-30

### CI

- Handle already-published crate error for `cargo publish` idempotency (#19)
- Rename release artifacts with platform names before upload
- Switch macOS release builds to `macos-latest` native runners (fixes cross-compilation issues) (#18)

## [0.5.0] - 2026-03-30

### Added

- `dk ops stats` — new subcommand that calls `GET /v1/ops/stats` and displays server version, state, total vectors, namespace count, and uptime (DAK-918)
- Bumps `dakera-client 0.6.2 → 0.8.6` to access `DakeraClient::ops_stats()` and `OpsStats`

### CI

- Migrate to self-hosted ARM runner for faster cross-compilation (DAK-910)
- Fix target directory race condition between parallel CI jobs (#15)
- Reduce GitHub Actions cost via zigbuild, concurrency limits, and paths-ignore (DAK-840)

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
