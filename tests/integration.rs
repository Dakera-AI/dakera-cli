//! Integration tests for dakera-cli.
//!
//! Each test spins up a local [`httpmock`] HTTP server and exercises the
//! compiled `dk` binary via [`assert_cmd`]. No running Dakera server is needed.
//!
//! Container integration tests (marked `#[ignore]`) require a running dakera
//! server. Run them with:
//!   DAKERA_TEST_URL=http://localhost:3300 DAKERA_TEST_KEY=test-key \
//!   cargo test --test integration -- --ignored
//!
//! Commands covered (httpmock):
//!   - `health`                       basic health check
//!   - `namespace list/policy`        namespace management
//!   - `memory store/recall/get/forget/update/search/importance/consolidate/feedback`
//!   - `agent list/stats/memories/sessions`
//!   - `session start/end/list/memories`
//!   - `vector upsert-one/delete`
//!   - `knowledge graph/deduplicate`
//!   - `keys list/create/delete`
//!   - Error responses (401, 500)

use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::prelude::*;
use serde_json::json;

fn dk() -> Command {
    Command::cargo_bin("dk").expect("dk binary not found — run `cargo build` first")
}

// ---------------------------------------------------------------------------
// health
// ---------------------------------------------------------------------------

#[test]
fn health_reports_healthy() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "healthy": true, "version": "0.5.2" }));
    });

    dk().args(["--url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("healthy"));
}

#[test]
fn health_unreachable_server_exits_with_failure() {
    dk().args(["--url", "http://127.0.0.1:1", "health"])
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// namespace list
// ---------------------------------------------------------------------------

#[test]
fn namespace_list_empty_shows_no_namespaces_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "namespaces": [] }));
    });

    dk().args(["--url", &server.base_url(), "namespace", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No namespaces found"));
}

#[test]
fn namespace_list_returns_namespace_names() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "namespaces": ["sdk-lead", "core-engine"] }));
    });

    let assert = dk()
        .args([
            "--url",
            &server.base_url(),
            "--format",
            "json",
            "namespace",
            "list",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("sdk-lead") && stdout.contains("core-engine"),
        "expected namespace names in JSON output, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// namespace policy get  (SEC-5 regression coverage)
// ---------------------------------------------------------------------------

#[test]
fn namespace_policy_get_prints_rate_limit_fields() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/namespaces/sdk-lead/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "working_ttl_seconds": 14400,
                "episodic_ttl_seconds": 2592000,
                "rate_limit_enabled": true,
                "rate_limit_stores_per_minute": 60,
                "rate_limit_recalls_per_minute": 120
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "namespace",
        "policy",
        "get",
        "sdk-lead",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("rate_limit_enabled"))
    .stdout(predicate::str::contains("true"));
}

// ---------------------------------------------------------------------------
// namespace policy set  (SEC-5 regression coverage)
// ---------------------------------------------------------------------------

#[test]
fn namespace_policy_set_rate_limit_reports_success() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/myns/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "working_ttl_seconds": 14400, "rate_limit_enabled": false }));
    });

    server.mock(|when, then| {
        when.method(PUT).path("/v1/namespaces/myns/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "working_ttl_seconds": 14400,
                "rate_limit_enabled": true,
                "rate_limit_stores_per_minute": 30
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "policy",
        "set",
        "myns",
        "--rate-limit-enabled",
        "true",
        "--rate-limit-stores-per-minute",
        "30",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Memory policy updated for namespace 'myns'",
    ));
}

#[test]
fn namespace_policy_set_rate_limit_disabled_succeeds() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/myns/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "rate_limit_enabled": true }));
    });
    server.mock(|when, then| {
        when.method(PUT).path("/v1/namespaces/myns/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "rate_limit_enabled": false }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "policy",
        "set",
        "myns",
        "--rate-limit-enabled",
        "false",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Memory policy updated for namespace 'myns'",
    ));
}

// ---------------------------------------------------------------------------
// memory store
// ---------------------------------------------------------------------------

#[test]
fn memory_store_returns_success_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "memory_id": "mem-001",
                "namespace": "test-agent"
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "test-agent",
        "This is a test memory",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory stored"))
    .stdout(predicate::str::contains("mem-001"));
}

#[test]
fn memory_store_with_importance_flag() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "memory_id": "mem-002", "namespace": "test-agent" }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "test-agent",
        "High priority memory",
        "--importance",
        "0.9",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory stored"));
}

#[test]
fn memory_store_server_error_exits_failure() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(500)
            .header("Content-Type", "application/json")
            .json_body(json!({ "error": "Internal server error" }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "test-agent",
        "This will fail",
    ])
    .assert()
    .failure();
}

// ---------------------------------------------------------------------------
// memory recall
// ---------------------------------------------------------------------------

#[test]
fn memory_recall_empty_shows_no_memories_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "memories": [], "total_found": 0 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "test-agent",
        "recent tasks",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
}

#[test]
fn memory_recall_returns_found_count() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "memories": [
                    {
                        "id": "mem-001",
                        "content": "Completed task X successfully",
                        "memory_type": "episodic",
                        "importance": 0.8,
                        "score": 0.95,
                        "agent_id": "test-agent"
                    }
                ],
                "total_found": 1
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "test-agent",
        "completed tasks",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Found 1 memories"));
}

#[test]
fn memory_recall_unauthorized_exits_failure() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(401)
            .header("Content-Type", "application/json")
            .json_body(json!({ "error": "Unauthorized" }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "test-agent",
        "query",
    ])
    .assert()
    .failure();
}

// ---------------------------------------------------------------------------
// memory forget
// ---------------------------------------------------------------------------

#[test]
fn memory_forget_success_reports_deleted_count() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "deleted_count": 1 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "test-agent",
        "mem-001",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Deleted 1 memory"));
}

// ---------------------------------------------------------------------------
// memory get
// ---------------------------------------------------------------------------

#[test]
fn memory_get_shows_memory_content() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/get/mem-001");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "id": "mem-001",
                "content": "Important finding about cats",
                "memory_type": "semantic",
                "importance": 0.9,
                "score": 0.0,
                "agent_id": "test-agent"
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "memory",
        "get",
        "test-agent",
        "mem-001",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("mem-001"));
}

// ---------------------------------------------------------------------------
// memory search
// ---------------------------------------------------------------------------

#[test]
fn memory_search_empty_shows_no_memories() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/search");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "memories": [], "total_found": 0 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "search",
        "test-agent",
        "cats",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
}

// ---------------------------------------------------------------------------
// memory update
// ---------------------------------------------------------------------------

#[test]
fn memory_update_success_reports_memory_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(PUT)
            .path("/v1/agents/test-agent/memories/mem-001");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "memory_id": "mem-001" }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "update",
        "test-agent",
        "mem-001",
        "--content",
        "Updated content here",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("updated"));
}

// ---------------------------------------------------------------------------
// memory consolidate
// ---------------------------------------------------------------------------

#[test]
fn memory_consolidate_dry_run_shows_preview() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/consolidate");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "consolidated_count": 5,
                "removed_count": 3,
                "new_memories": []
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "consolidate",
        "test-agent",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[dry-run]"));
}

// ---------------------------------------------------------------------------
// memory feedback
// ---------------------------------------------------------------------------

#[test]
fn memory_feedback_submits_and_reports_status() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/agents/test-agent/memories/feedback");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "status": "accepted", "updated_importance": 0.75 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "feedback",
        "test-agent",
        "mem-001",
        "Very relevant",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Feedback submitted"));
}

// ---------------------------------------------------------------------------
// agent list
// ---------------------------------------------------------------------------

#[test]
fn agent_list_empty_shows_no_agents_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!([]));
    });

    dk().args(["--url", &server.base_url(), "agent", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No agents found"));
}

#[test]
fn agent_list_returns_agent_ids() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!([
                {
                    "agent_id": "core-engine",
                    "memory_count": 42,
                    "session_count": 10,
                    "active_sessions": 1
                }
            ]));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "agent",
        "list",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("core-engine"));
}

// ---------------------------------------------------------------------------
// agent stats
// ---------------------------------------------------------------------------

#[test]
fn agent_stats_shows_statistics_table() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents/core-engine/stats");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "agent_id": "core-engine",
                "total_memories": 42,
                "total_sessions": 10,
                "active_sessions": 1,
                "avg_importance": 0.75,
                "oldest_memory_at": null,
                "newest_memory_at": null,
                "memories_by_type": { "episodic": 30, "semantic": 12 }
            }));
    });

    dk().args(["--url", &server.base_url(), "agent", "stats", "core-engine"])
        .assert()
        .success()
        .stdout(predicate::str::contains("core-engine"))
        .stdout(predicate::str::contains("42"));
}

// ---------------------------------------------------------------------------
// agent memories
// ---------------------------------------------------------------------------

#[test]
fn agent_memories_empty_shows_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents/test-agent/memories");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!([]));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "agent",
        "memories",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
}

#[test]
fn agent_memories_returns_count() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents/test-agent/memories");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!([
                {
                    "id": "mem-001",
                    "content": "Test memory",
                    "memory_type": "episodic",
                    "importance": 0.8
                }
            ]));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "agent",
        "memories",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Found 1 memories"));
}

// ---------------------------------------------------------------------------
// agent sessions
// ---------------------------------------------------------------------------

#[test]
fn agent_sessions_empty_shows_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/agents/test-agent/sessions");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!([]));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "agent",
        "sessions",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No sessions found"));
}

// ---------------------------------------------------------------------------
// session start
// ---------------------------------------------------------------------------

#[test]
fn session_start_prints_session_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/start");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "session": {
                    "id": "sess-abc123",
                    "agent_id": "test-agent",
                    "started_at": 1716000000_u64
                }
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "start",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Session started"))
    .stdout(predicate::str::contains("sess-abc123"));
}

// ---------------------------------------------------------------------------
// session end
// ---------------------------------------------------------------------------

#[test]
fn session_end_prints_confirmation() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/sess-abc123/end");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "session": {
                    "id": "sess-abc123",
                    "agent_id": "test-agent",
                    "started_at": 1716000000_u64,
                    "ended_at": 1716001000_u64
                },
                "memory_count": 3
            }));
    });

    dk().args(["--url", &server.base_url(), "session", "end", "sess-abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sess-abc123"))
        .stdout(predicate::str::contains("ended"));
}

// ---------------------------------------------------------------------------
// session list
// ---------------------------------------------------------------------------

#[test]
fn session_list_empty_shows_no_sessions_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/sessions");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "sessions": [], "total": 0 }));
    });

    dk().args(["--url", &server.base_url(), "session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No sessions found"));
}

#[test]
fn session_list_shows_session_count() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/sessions");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "sessions": [
                    {
                        "id": "sess-001",
                        "agent_id": "test-agent",
                        "started_at": 1716000000_u64,
                        "ended_at": null
                    }
                ],
                "total": 1
            }));
    });

    dk().args(["--url", &server.base_url(), "session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sess-001"));
}

// ---------------------------------------------------------------------------
// session memories
// ---------------------------------------------------------------------------

#[test]
fn session_memories_empty_shows_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/sessions/sess-001/memories");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "memories": [], "total_found": 0 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "memories",
        "sess-001",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
}

// ---------------------------------------------------------------------------
// vector upsert-one
// ---------------------------------------------------------------------------

#[test]
fn vector_upsert_one_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/namespaces/test-ns/vectors");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "upserted_count": 1 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "vector",
        "upsert-one",
        "--namespace",
        "test-ns",
        "--id",
        "vec-001",
        "--values",
        "0.1,0.2,0.3",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("vec-001"));
}

// ---------------------------------------------------------------------------
// vector delete
// ---------------------------------------------------------------------------

#[test]
fn vector_delete_by_ids_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/test-ns/vectors/delete");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "deleted_count": 2 }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "vector",
        "delete",
        "--namespace",
        "test-ns",
        "--ids",
        "vec-001,vec-002",
        "--yes",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Deleted 2 vectors"));
}

#[test]
fn vector_delete_dry_run_skips_server_call() {
    // dry-run should print message and exit 0 without contacting server
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "vector",
        "delete",
        "--namespace",
        "test-ns",
        "--ids",
        "vec-001",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[dry-run]"));
}

// ---------------------------------------------------------------------------
// knowledge graph
// ---------------------------------------------------------------------------

#[test]
fn knowledge_graph_empty_shows_zero_nodes() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/knowledge/graph");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "nodes": [],
                "edges": [],
                "clusters": null
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "graph",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("0 nodes"));
}

#[test]
fn knowledge_graph_shows_node_and_edge_counts() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/knowledge/graph");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "nodes": [
                    {
                        "id": "mem-001",
                        "content": "Cat3 temporal reasoning memory",
                        "memory_type": "semantic",
                        "importance": 0.9
                    }
                ],
                "edges": [
                    {
                        "source": "mem-001",
                        "target": "mem-002",
                        "similarity": 0.85,
                        "relationship": "similar"
                    }
                ],
                "clusters": null
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "graph",
        "test-agent",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("1 nodes"))
    .stdout(predicate::str::contains("1 edges"));
}

// ---------------------------------------------------------------------------
// knowledge deduplicate
// ---------------------------------------------------------------------------

#[test]
fn knowledge_deduplicate_dry_run_reports_found_groups() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/knowledge/deduplicate");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "duplicates_found": 4,
                "groups": [["mem-001", "mem-002"]],
                "removed_count": 2
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "deduplicate",
        "test-agent",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[dry-run]"));
}

// ---------------------------------------------------------------------------
// keys list
// ---------------------------------------------------------------------------

#[test]
fn keys_list_shows_api_keys() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/keys");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "keys": [
                    { "key_id": "key-abc", "name": "ci-key", "scope": "read", "active": true }
                ]
            }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "keys",
        "list",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("API Keys"));
}

// ---------------------------------------------------------------------------
// keys create
// ---------------------------------------------------------------------------

#[test]
fn keys_create_shows_new_key_value() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/admin/keys");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({
                "key": "dk_secret_test_key_value",
                "key_id": "key-001",
                "name": "test-key"
            }));
    });

    dk().args(["--url", &server.base_url(), "keys", "create", "test-key"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-key"))
        .stdout(predicate::str::contains("dk_secret_test_key_value"));
}

// ---------------------------------------------------------------------------
// keys delete
// ---------------------------------------------------------------------------

#[test]
fn keys_delete_success_reports_deletion() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/admin/keys/key-abc");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({}));
    });

    dk().args(["--url", &server.base_url(), "keys", "delete", "key-abc"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted"));
}

// ---------------------------------------------------------------------------
// Container integration tests
// These require a running dakera server. Run with:
//   cargo test --test integration -- --ignored
// ---------------------------------------------------------------------------

fn container_dk(url: &str, key: &str) -> Command {
    let mut cmd = Command::cargo_bin("dk").expect("dk binary not found");
    if !key.is_empty() {
        cmd.env("DAKERA_API_KEY", key);
    }
    cmd.arg("--url").arg(url);
    cmd
}

fn container_url() -> String {
    std::env::var("DAKERA_TEST_URL").unwrap_or_else(|_| "http://localhost:3300".to_string())
}

fn container_key() -> String {
    std::env::var("DAKERA_TEST_KEY").unwrap_or_default()
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_health_check() {
    let url = container_url();
    let key = container_key();

    container_dk(&url, &key)
        .arg("health")
        .assert()
        .success()
        .stdout(predicate::str::contains("healthy"));
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_namespace_create_and_list() {
    let url = container_url();
    let key = container_key();

    // 'namespace create' is a no-op that informs the user that namespaces
    // are created implicitly on first vector upsert — verify the message.
    container_dk(&url, &key)
        .args(["namespace", "create", "integration-test-ns"])
        .assert()
        .success()
        .stdout(predicate::str::contains("integration-test-ns"));

    // 'namespace list' should succeed (empty is fine on a fresh server).
    container_dk(&url, &key)
        .args(["namespace", "list"])
        .assert()
        .success();
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_memory_store_and_recall() {
    let url = container_url();
    let key = container_key();

    // Store a memory
    container_dk(&url, &key)
        .args([
            "memory",
            "store",
            "integration-agent",
            "Container integration test memory — temporal reasoning",
            "--importance",
            "0.8",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Memory stored"));

    // Recall it
    container_dk(&url, &key)
        .args([
            "memory",
            "recall",
            "integration-agent",
            "temporal reasoning",
        ])
        .assert()
        .success();
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_memory_forget() {
    let url = container_url();
    let key = container_key();

    // Store a memory first
    let assert = container_dk(&url, &key)
        .args([
            "memory",
            "store",
            "integration-agent",
            "Memory to be forgotten",
        ])
        .assert()
        .success();

    // Parse out the memory ID from stdout
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let id = stdout
        .split("id: ")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .expect("could not parse memory ID from store output")
        .to_string();

    // Forget it
    container_dk(&url, &key)
        .args(["memory", "forget", "integration-agent", id.trim()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted"));
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_session_lifecycle() {
    let url = container_url();
    let key = container_key();

    // Start a session
    let assert = container_dk(&url, &key)
        .args(["session", "start", "integration-agent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Session started"));

    // Parse session ID
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let session_id = stdout
        .split("id: ")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .or_else(|| {
            stdout
                .split("id: ")
                .nth(1)
                .and_then(|s| s.split(')').next())
        })
        .expect("could not parse session ID")
        .trim()
        .to_string();

    // End the session
    container_dk(&url, &key)
        .args(["session", "end", session_id.trim()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ended"));
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_agent_list() {
    let url = container_url();
    let key = container_key();

    container_dk(&url, &key)
        .args(["agent", "list"])
        .assert()
        .success();
}

#[test]
#[ignore = "requires running dakera container (set DAKERA_TEST_URL)"]
fn container_vector_operations() {
    let url = container_url();
    let key = container_key();

    // Upsert a single vector (3-dim for simplicity)
    container_dk(&url, &key)
        .args([
            "vector",
            "upsert-one",
            "--namespace",
            "integration-test-ns",
            "--id",
            "integration-vec-001",
            "--values",
            "0.1,0.2,0.3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("integration-vec-001"));
}
