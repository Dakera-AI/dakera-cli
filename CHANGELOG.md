# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.1] - 2026-10-08

Support for Dakera server **v0.12.2** (Dakera-AI/dakera#916). Compatible with v0.11.108,
v0.12.0, v0.12.1 and v0.12.2 servers; the commands below that need v0.12.2 say so on an
older server (see the README compatibility table). Built on `dakera-client` 0.12.2.

### Added

- **`dk whoami`**: `GET /v1/auth/whoami` — the key the CLI authenticates with (scope,
  grants, unrestricted, grant syntax version, inert grants).
- **`dk agent create <agent_id>`**: `POST /v1/agents` — create an agent before its first
  memory; idempotent (an existing agent is reported and left unchanged).
- **`dk keys edit <key_id>`** (alias `patch`): `PATCH /admin/keys/{id}`, or with
  `-n <namespace>` `PATCH /v1/namespaces/{ns}/keys/{id}` — `--name`, `--namespaces a,b`
  (`p*` prefix patterns allowed), `--all-namespaces` (sends `null`), `--no-namespaces`
  (sends `[]`). Nothing to change exits 5 without a request.
- **`dk keys rotate --grace <duration>`**: the old key keeps working for up to 7 days; the
  new key id and the old key's deadline are shown.
- **`dk session touch <session_id>`**: `POST /v1/sessions/{id}/touch` — keep a quiet session
  from being ended for inactivity (an ended session is reported, never re-opened).
- **`dk session start --idle-timeout <duration>`**: the session's own inactivity timeout
  (`0` = never, at most 30 days).
- **`--preview <chars>`** on `dk agent memories`, `dk session memories` and
  `dk knowledge full-graph` (`content_preview_chars`, 1-10000): the server cuts each content;
  the listings add `CONTENT_LEN` and `TRUNCATED` columns. **`--include-derived`** on
  `dk agent memories`. `--offset` on both memory listings, `--limit` on `dk session memories`.
- **`dk admin derivations-status`** and **`dk admin derivations-drain [--timeout]`**:
  `GET /admin/derivations/status`, `POST /admin/derivations/drain` (global admin key).
- **`dk admin session-idle-timeout [<duration>]`**: show or set the server-wide session
  inactivity timeout (`session_idle_timeout_secs` of `/admin/config`).
- Durations accept `s`, `m`, `h`, `d` suffixes (`3600`, `90m`, `8h`, `7d`).
- `dk session list` shows `ended_reason` (`client` / `idle`).
- Shell completions list the new commands.

### Changed

- `dk session memories` reports the server's `total` (it printed 0) and reads the listing
  through `dakera-client` 0.12.2.
- `ClientError::InvalidRequest` from the SDK exits 5 (invalid input).

### Fixed

- `dk keys rotate` printed no new key: it looked for `key`, the server answers `new_key`.

### Server behaviour changes you may hit (Dakera v0.12.2)

- **Sessions are authorized by their agent**: keys no longer need a `_dakera_sessions` grant
  (such an entry is inert, see `dk whoami`); a key without grants lists no sessions;
  `dk session end` with a read-only key gets `403` (exit 4), and ending a session of an
  agent the key cannot reach returns the empty idempotent answer.
- **Sessions auto-end after 4 h idle by default** (`ended_reason: idle`). Use
  `dk session touch`, `dk session start --idle-timeout`, or
  `dk admin session-idle-timeout`. Storing into an ended session still succeeds.
- **Stricter validation** (`400`, exit 5, the message names the field): key namespace
  lists, agent ids (at most 241 bytes), memory metadata, reserved markers, TTLs, imports.
- **The memory content limit is in bytes** (UTF-8, default 100000), also on `dk memory update`.
- **Listings exclude derived records unless `include_derived=true`**: `dk agent memories`
  no longer shows the sentence sub-memories; add `--include-derived`.

### Dependency

- `dakera-client` 0.12.2 (crates.io).

## [0.8.0] - 2026-10-01

Support for Dakera server v0.12.0. Compatible with v0.11.108 and v0.12.0 servers;
the commands below that call v0.12 routes say so when run against v0.11.

### Added

- **`dk capabilities`**: `GET /v1/capabilities` as a table (active model, search
  mode, scoring strategy, query languages, opt-in features on/off). Exits 3 with a
  clear message on a server that predates v0.12.
- **`dk health ready` / `dk health live`**: the readiness and liveness probes.
  `dk health` and `dk health --detailed` now read the JSON directly: a starting
  server (`503` + `Retry-After`) is reported as such (exit 6) instead of healthy,
  and the `degraded`, `config_warnings` and `embed_migration` fields are shown.
- **`dk attachment upload|list|download|delete|transcribe|index|job`**: the v0.12
  attachment, speech-to-text and image-index routes (opt-in on the server:
  `DAKERA_ATTACHMENTS`, `DAKERA_VISION`), with `--wait` for the background jobs.
- **`dk admin embed-migration`**: `GET /admin/reembed/migration`.
- **`dk admin encryption-status|encryption-rotate|encryption-reseal`**: the v0.12
  keyring; rotation of one namespace (`-n`) or of everything, the new key read from
  an environment variable (`--new-key-env`), never from the command line.
- **`dk admin backup-get|backup-download|backup-upload|backup-restore-status|backup-schedule`**,
  and options on `backup-create` (`--name`, `--type`, `-n`, `--encrypt`,
  `--compression`, `--wait`) and `backup-restore` (`-n`, `--overwrite --yes`, `--wait`).
- **Permission and error messages**: every non-2xx answer keeps the server's JSON
  error body. A `403` says whether the key is pinned to namespaces (v0.12: `403` on
  node-wide routes) or lacks `super_admin` (backup download, upload, restore); a
  `413`, `501` and `503` say what to change or when to retry (`Retry-After`).
  With `--format json` the error gains `http_status`, `server_code`, `details`,
  `retry_after_secs`.
- **`--lang`** on `dk memory store|recall|search|update` (server v0.12): the language
  of the content or query (ISO 639-1 code or name, optionally with a region, e.g.
  `pt-BR`); the server lists what it supports in `dk capabilities`.
- **`--attachment-ref`** on `dk memory store` (server v0.12, `DAKERA_ATTACHMENTS`): link
  the memory to an uploaded attachment (`sha256:<hex>`).
- **`dk memory batch-store`**: `POST /v1/memories/store/batch`, up to 1000 memories
  in one request from repeated `--content` and/or a `--file` JSON array (`-` reads
  stdin); `--type`, `--importance`, `--tag`, `--session-id` fill in what an item does
  not set, and `--lang` applies to the whole batch.
- **`dk memory extract`**: `POST /v1/memories/extract`, entity extraction without
  storing (`--entity-types`, `--lang`).
- README: what is new, compatibility with v0.11.108 and v0.12.0, permissions, and
  the server-side `dakera downgrade` and `dakera --check-config` commands.

### Changed

- Exit codes follow the HTTP status of an error answer: `401`/`403` exit 4;
  `400`/`409`/`413`/`415`/`422` exit 5; every 5xx (including `501` and `503`) exits 6.
- Shell completions list the new commands.
- `dakera-client` 0.11 -> 0.12.0.
- `dk memory store` prints the agent instead of `namespace: default` (the store
  answer carries no namespace; that value was a placeholder).
- `dk memory recall|search` print the server's total only when it reports one larger
  than the page (recall reports none, so it showed `total: 0`).

### Fixed

- `dk admin backup-create` sent `{"include_data": ...}` and no `name`, which the
  server requires, so it could not succeed; it now sends `name` (default
  `dk-backup-<unix time>`). The `--no-data` flag, which the server never read, is gone.
- `dk admin quotas-set` called `PUT /admin/quotas`, a route that does not exist; it
  now calls `/admin/quotas/{namespace}` (`-n`) or `/admin/quotas/default`.
- **Calls to routes the v0.12.0 server does not serve** (found by diffing every route `dk` calls, directly
  and through `dakera-client` 0.11, against the server router):
  - `dk memory update` called `PUT /v1/agents/{agent}/memories/{id}`; it now calls `PUT /v1/memory/update/{id}?agent_id=`.
  - `dk memory feedback` called `POST /v1/agents/{agent}/memories/feedback` with free text; it now calls
    `POST /v1/memory/feedback` with a `signal` (`upvote`, `downvote`, `flag`, `positive`, `negative`). The text and `--score` arguments are gone.
  - `dk memory batch-forget` called `POST /v1/memories/forget/batch` with fields the server does not read; it now calls
    `DELETE` with `{agent_id, filter}` (`--max-age-days` becomes `created_before`) and `--dry-run` counts matches through `POST /v1/memories/recall/batch`.
  - `dk text search` called `POST /v1/fulltext/search`; it now calls `POST /v1/namespaces/{ns}/fulltext/search` and **needs `--namespace`**.
  - `dk admin configure-ttl` called `PUT /admin/namespaces/{ns}/ttl`, which does not exist, and is removed: use `dk namespace policy set` (TTL fields).
- `dk admin` commands printed their success line before checking the answer, so a
  refused call showed a green check and then the error.
- The SDK-backed commands (`dk memory`, `dk session`, ...) now get the same exit codes
  by status as the others: a `400` exited 6 and a `501` exited 1.
- **`dk knowledge graph|full-graph|summarize|deduplicate` and `dk index fulltext-stats`
  failed on every call** against v0.11.108 and v0.12.0 (`dakera-client` does not match
  these routes: "error decoding response body", or a 422). They now call the REST API
  with the server's request and answer shapes:
  - `dk knowledge graph` needs `--memory-id` (the server builds the graph around a seed
    memory) and prints the seed and its related memories.
  - `dk knowledge summarize` needs `--memory-ids` with at least two ids, prints the new
    summary memory, and **`--dry-run` is removed**: the server has no dry run and always
    stores the summary, so the flag announced a preview while writing.
  - `dk knowledge full-graph` prints clusters and the hub memory; `deduplicate` prints
    each group's canonical id, duplicates and similarity.
  The container tests for these commands accepted the failures ("response schema may
  differ"); they now assert success against the real server.
- **Commands that printed a result without calling the server** now do the work:
  - `dk namespace create <ns> --dimension <N> [--distance cosine|euclidean|dot]` creates
    the namespace (`PUT /v1/namespaces/{ns}`); it printed "will be created on first vector
    upsert" and pointed to a `dk vector upsert` command that does not exist.
    **`--dimension` is now required** (the server needs it).
  - `dk namespace delete` deletes (`DELETE /v1/namespaces/{ns}`); it said the server could
    not delete namespaces.
  - `dk index rebuild` rebuilds (`POST /admin/indexes/rebuild`, new `--force`); it said
    "not yet available". `--index-type` no longer defaults to `all` (not a server value):
    the server picks flat or HNSW per namespace and rejects a different expected kind.
- `dk agent stats` failed to decode the server's integer timestamps.
- Shell completions offered `vector`, `ops` and `analytics`, commands that were removed in
  0.6; `dk init` pointed to vector upserts `dk` cannot do.

## [0.6.0] - 2026-05-20

### Added

- **Aligned table output** (`comfy-table v7`): `--format table` now renders properly aligned
  columns with bold cyan headers instead of falling back to JSON pretty-print.
- **Progress bar for bulk upsert** (`indicatif v0.17`): `dk vector upsert --file big.json`
  shows a spinner, elapsed time, item count, and ETA for large batches.
- **Verbose HTTP logging** (`--verbose` flag): all commands now log `-->` request and `<--`
  response lines with elapsed milliseconds via the `Context` struct and `tracing`.
- **Exponential backoff retry** (`src/retry.rs`): transient network errors are retried up to
  3 times with delays of 100 ms / 500 ms / 2 s; 4xx client errors are never retried.
- `src/context.rs`: new `Context` struct threading `url`, `format`, and `verbose` through
  all command modules — eliminates per-call `url`/`format` argument threading.
- `src/cli.rs`: all `build_*_command()` builder functions extracted from `main.rs`.

### Changed

- `src/main.rs` reduced from ~1,400 lines to ~125 lines (routing + init only).
- All command modules updated to accept `&Context` instead of `(url: &str, ..., format)`.
- `.gitignore` extended to exclude `*.db` and `ruvector.db` test artifacts.

### Dependencies

- Added `comfy-table = "7"` for aligned column rendering.
- Added `indicatif = "0.17"` for progress bars.

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
