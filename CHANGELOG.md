# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
