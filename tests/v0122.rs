//! Integration tests for the Dakera v0.12.2 commands of `dk`.
//!
//! Like `v012.rs`, each test starts a local [`httpmock`] server and runs the
//! compiled `dk` binary against it; no Dakera server is needed.
//!
//! Covered: `agent create`, `agent memories --preview --include-derived`,
//! `session start --idle-timeout`, `session touch`, `session memories
//! --preview`, `keys edit`, `keys rotate --grace`, `whoami`,
//! `admin derivations-status|derivations-drain|session-idle-timeout` and
//! `knowledge full-graph --preview`, plus what a pre-v0.12.2 server answers.

use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::prelude::*;
use serde_json::json;

fn dk() -> Command {
    let mut cmd = Command::cargo_bin("dk").expect("dk binary not found — run `cargo build` first");
    cmd.env_remove("DAKERA_API_KEY");
    cmd
}

const KEY_INFO: &str = r#"{"key_id":"dk_key_1","name":"ci","scope":"write","namespaces":["team-*"],
    "created_at":1791392203,"expires_at":null,"active":true,"grants_version":1}"#;

// ---------------------------------------------------------------------------
// agent create / memories
// ---------------------------------------------------------------------------

#[test]
fn agent_create_reports_created() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/agents")
            .json_body(json!({"agent_id": "mlx-dev"}));
        then.status(201).json_body(json!({
            "agent_id": "mlx-dev", "namespace": "_dakera_agent_mlx-dev",
            "created": true, "dimension": 1024, "model": "bge-large"
        }));
    });
    dk().args(["--url", &server.base_url(), "agent", "create", "mlx-dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent 'mlx-dev' created"))
        .stdout(predicate::str::contains("_dakera_agent_mlx-dev"))
        .stdout(predicate::str::contains("bge-large"));
    m.assert();
}

#[test]
fn agent_create_existing_says_unchanged_and_json_prints_answer() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/agents");
        then.status(200).json_body(json!({
            "agent_id": "mlx-dev", "namespace": "_dakera_agent_mlx-dev",
            "created": false, "dimension": null, "model": "bge-large"
        }));
    });
    dk().args(["--url", &server.base_url(), "agent", "create", "mlx-dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "agent",
        "create",
        "mlx-dev",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"created\": false"));
}

#[test]
fn agent_create_on_a_pre_v0122_server_exits_with_an_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/agents");
        then.status(405).body("");
    });
    dk().args(["--url", &server.base_url(), "agent", "create", "mlx-dev"])
        .assert()
        .failure();
}

#[test]
fn agent_memories_sends_preview_and_include_derived() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/agents/a/memories")
            .query_param("limit", "50")
            .query_param("include_derived", "true")
            .query_param("content_preview_chars", "5");
        then.status(200).json_body(json!([{
            "id": "m1", "agent_id": "a", "content": "Hello", "memory_type": "semantic",
            "importance": 0.5, "created_at": 1, "last_accessed_at": 1, "access_count": 0,
            "content_len": 42, "content_truncated": true
        }]));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "agent",
        "memories",
        "a",
        "--include-derived",
        "--preview",
        "5",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("CONTENT_LEN"))
    .stdout(predicate::str::contains("42"))
    .stdout(predicate::str::contains("1 memories are previews"));
    m.assert();
}

#[test]
fn agent_memories_without_flags_sends_no_new_parameters() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/agents/a/memories")
            .query_param_missing("include_derived")
            .query_param_missing("content_preview_chars");
        then.status(200).json_body(json!([{
            "id": "m1", "content": "full", "memory_type": "semantic", "importance": 0.5,
            "created_at": 1, "last_accessed_at": 1, "access_count": 0
        }]));
    });
    dk().args(["--url", &server.base_url(), "agent", "memories", "a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CONTENT_LEN").not());
    m.assert();
}

// ---------------------------------------------------------------------------
// sessions
// ---------------------------------------------------------------------------

#[test]
fn session_start_sends_idle_timeout() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sessions/start")
            .json_body(json!({"agent_id": "a", "idle_timeout_secs": 7200}));
        then.status(200).json_body(json!({"session": {
            "id": "sess_1", "agent_id": "a", "started_at": 10, "memory_count": 0,
            "last_activity_at": 10, "idle_timeout_secs": 7200
        }}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "start",
        "a",
        "--idle-timeout",
        "2h",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Session started (id: sess_1"));
    m.assert();
}

#[test]
fn session_start_idle_timeout_over_30_days_exits_5_without_a_request() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/start");
        then.status(200);
    });
    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "start",
        "a",
        "--idle-timeout",
        "31d",
    ])
    .assert()
    .code(5);
    m.assert_calls(0);
}

#[test]
fn session_touch_active_and_ended() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/sess_a/touch");
        then.status(200).json_body(json!({
            "session": {"id": "sess_a", "agent_id": "a", "started_at": 1, "last_activity_at": 1791393000},
            "session_state": "active", "idle_deadline_at": 1791407400
        }));
    });
    server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/sess_b/touch");
        then.status(200).json_body(json!({
            "session": {"id": "sess_b", "agent_id": "a", "started_at": 1, "ended_at": 5,
                        "ended_reason": "idle", "idle_since": 2, "last_activity_at": 2},
            "session_state": "ended"
        }));
    });
    dk().args(["--url", &server.base_url(), "session", "touch", "sess_a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("touched"))
        .stdout(predicate::str::contains("1791407400"));
    dk().args(["--url", &server.base_url(), "session", "touch", "sess_b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already ended (idle)"));
}

#[test]
fn session_touch_unknown_session_exits_3() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/sessions/nope/touch");
        then.status(404).json_body(json!({
            "error": "Session not found: nope", "code": "NOT_FOUND", "status": 404
        }));
    });
    dk().args(["--url", &server.base_url(), "session", "touch", "nope"])
        .assert()
        .code(3);
}

#[test]
fn session_memories_sends_preview_and_shows_total() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sessions/sess_1/memories")
            .query_param("content_preview_chars", "3")
            .query_param("limit", "1");
        then.status(200).json_body(json!({
            "session": {"id": "sess_1", "agent_id": "a", "started_at": 1},
            "memories": [{"id": "m1", "agent_id": "a", "content": "abc", "memory_type": "episodic",
                          "importance": 0.5, "created_at": 1, "last_accessed_at": 1, "access_count": 0,
                          "content_len": 300, "content_truncated": true}],
            "total": 7
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "memories",
        "sess_1",
        "--preview",
        "3",
        "--limit",
        "1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("(total: 7)"))
    .stdout(predicate::str::contains("300"));
    m.assert();
}

#[test]
fn session_list_shows_ended_reason() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/sessions");
        then.status(200).json_body(json!({
            "sessions": [{"id": "sess_1", "agent_id": "a", "started_at": 1, "ended_at": 9, "ended_reason": "idle"}],
            "total": 1
        }));
    });
    dk().args(["--url", &server.base_url(), "session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ENDED_REASON"))
        .stdout(predicate::str::contains("idle"));
}

// ---------------------------------------------------------------------------
// keys
// ---------------------------------------------------------------------------

#[test]
fn keys_edit_all_namespaces_sends_null() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method("PATCH")
            .path("/admin/keys/dk_key_1")
            .json_body(json!({"namespaces": null}));
        then.status(200).body(KEY_INFO);
    });
    dk().args([
        "--url",
        &server.base_url(),
        "keys",
        "edit",
        "dk_key_1",
        "--all-namespaces",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("API key 'dk_key_1' updated"))
    .stdout(predicate::str::contains("team-*"));
    m.assert();
}

#[test]
fn keys_patch_through_a_namespace_sends_name_and_list() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method("PATCH")
            .path("/v1/namespaces/team-a/keys/dk_key_1")
            .json_body(json!({"name": "ci", "namespaces": ["team-a*", "docs"]}));
        then.status(200).body(KEY_INFO);
    });
    dk().args([
        "--url",
        &server.base_url(),
        "keys",
        "patch",
        "dk_key_1",
        "-n",
        "team-a",
        "--name",
        "ci",
        "--namespaces",
        "team-a*,docs",
    ])
    .assert()
    .success();
    m.assert();
}

#[test]
fn keys_edit_without_changes_exits_5_without_a_request() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method("PATCH").path("/admin/keys/dk_key_1");
        then.status(200).body(KEY_INFO);
    });
    dk().args(["--url", &server.base_url(), "keys", "edit", "dk_key_1"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("nothing to change"));
    m.assert_calls(0);
}

#[test]
fn keys_edit_conflict_409_exits_5() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("PATCH").path("/admin/keys/dk_key_1");
        then.status(409).json_body(json!({
            "error": "API key 'dk_key_1' is inactive", "code": "CONFLICT", "status": 409
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "keys",
        "edit",
        "dk_key_1",
        "--name",
        "x",
    ])
    .assert()
    .code(5);
}

#[test]
fn keys_rotate_with_grace_sends_grace_and_shows_old_key_deadline() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/keys/dk_key_old/rotate")
            .json_body(json!({"grace_secs": 3600}));
        then.status(200).json_body(json!({
            "new_key": "dk_newsecret", "key_id": "dk_key_new", "old_key_id": "dk_key_old",
            "old_key_expires_at": 1791395803, "warning": "Save this new key now!"
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "keys",
        "rotate",
        "dk_key_old",
        "--grace",
        "1h",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("New Key: dk_newsecret"))
    .stdout(predicate::str::contains("New Key ID: dk_key_new"))
    .stdout(predicate::str::contains("keeps working until 1791395803"));
    m.assert();
}

#[test]
fn keys_rotate_without_grace_shows_the_new_key_of_a_v0121_answer() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/keys/dk_key_old/rotate")
            .body("");
        then.status(200).json_body(json!({
            "new_key": "dk_newsecret", "key_id": "dk_key_new", "warning": "w"
        }));
    });
    dk().args(["--url", &server.base_url(), "keys", "rotate", "dk_key_old"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New Key: dk_newsecret"));
    m.assert();
}

#[test]
fn keys_rotate_grace_over_seven_days_exits_5() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "keys",
        "rotate",
        "k",
        "--grace",
        "8d",
    ])
    .assert()
    .code(5);
}

#[test]
fn whoami_table_and_json() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/whoami");
        then.status(200).json_body(json!({
            "key_id": "dk_key_1", "name": "dev", "scope": "write", "namespaces": ["foo*", "docs"],
            "unrestricted": false, "expires_at": null, "grants_version": 0,
            "inert_namespaces": ["foo*"], "auth_enabled": true
        }));
    });
    dk().args(["--url", &server.base_url(), "whoami"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dk_key_1"))
        .stdout(predicate::str::contains("foo*, docs"))
        .stdout(predicate::str::contains("Inert grants"))
        .stdout(predicate::str::contains("predates prefix patterns"));
    dk().args(["--url", &server.base_url(), "--format", "json", "whoami"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"grants_version\": 0"));
}

#[test]
fn whoami_on_a_pre_v0122_server_exits_3() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/whoami");
        then.status(404)
            .json_body(json!({"error": "Not found", "code": "NOT_FOUND", "status": 404}));
    });
    dk().args(["--url", &server.base_url(), "whoami"])
        .assert()
        .code(3);
}

// ---------------------------------------------------------------------------
// admin
// ---------------------------------------------------------------------------

fn derivation_status(settled: bool) -> serde_json::Value {
    json!({
        "settled": settled, "pending_sentences": if settled { 0 } else { 4 }, "pending_parents": 0,
        "unmarked_parents": 0, "stale_children": 0, "orphan_children": 0, "remeta_children": 0,
        "duplicate_children": 0, "legacy_children": 0, "bm25_missing": 0, "graph_owed": 0,
        "in_flight": 0, "graph_queue_owed": 0, "dirty_namespaces": [], "namespaces": 3,
        "unreadable_namespaces": ["_dakera_agent_z"],
        "heal": {"version": 1, "complete": true, "namespace": null, "cursor": null,
                 "parents_healed": 12, "graph_adopted": 0, "started_at": 1, "completed_at": 2},
        "reconciler": {"state": "sleeping", "last_tick_at": 100, "ticks": 7, "next_namespace": null},
        "counters": {"derived": 5}
    })
}

#[test]
fn admin_derivations_status_shows_owed_counts() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/admin/derivations/status");
        then.status(200).json_body(derivation_status(false));
    });
    dk().args(["--url", &server.base_url(), "admin", "derivations-status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("still owed"))
        .stdout(predicate::str::is_match(r"Pending sentences\S*: 4").unwrap())
        .stdout(predicate::str::contains("complete (12 parents healed)"))
        .stdout(predicate::str::contains("_dakera_agent_z"));
}

#[test]
fn admin_derivations_drain_sends_timeout() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/admin/derivations/drain")
            .json_body(json!({"timeout_secs": 60}));
        then.status(200).json_body(json!({
            "settled": true, "timed_out": false, "rounds": 1, "elapsed_ms": 1234,
            "parents_run": 17, "pending_left": 0, "deleted": 3, "bm25_restored": 0,
            "graph_queued": 1, "status": derivation_status(true)
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "derivations-drain",
        "--timeout",
        "1m",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Drained in 1234 ms"))
    .stdout(predicate::str::is_match(r"Parents run\S*: 17").unwrap());
    m.assert();
}

#[test]
fn admin_derivations_drain_conflict_exits_5() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/admin/derivations/drain");
        then.status(409).json_body(json!({
            "error": "a derivation drain is already running", "code": "CONFLICT", "status": 409
        }));
    });
    dk().args(["--url", &server.base_url(), "admin", "derivations-drain"])
        .assert()
        .code(5);
}

fn config(timeout: Option<u64>) -> serde_json::Value {
    let mut c = json!({
        "default_index_type": "hnsw", "cache_enabled": true, "cache_max_size_bytes": 1,
        "rate_limit_enabled": false, "rate_limit_rps": 0, "query_timeout_ms": 1000
    });
    if let Some(t) = timeout {
        c["session_idle_timeout_secs"] = json!(t);
    }
    c
}

#[test]
fn admin_session_idle_timeout_show_and_set() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/admin/config");
        then.status(200).json_body(config(Some(14_400)));
    });
    let put = server.mock(|when, then| {
        when.method(PUT)
            .path("/v1/admin/config")
            .json_body(json!({"session_idle_timeout_secs": 28_800}));
        then.status(200).json_body(json!({
            "success": true, "config": config(Some(28_800)), "message": "ok"
        }));
    });
    dk().args(["--url", &server.base_url(), "admin", "session-idle-timeout"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"Session idle timeout\S*: 4h").unwrap());
    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "session-idle-timeout",
        "8h",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("set to 8h"));
    put.assert();
}

#[test]
fn admin_session_idle_timeout_on_a_pre_v0122_server_says_so() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/admin/config");
        then.status(200).json_body(config(None));
    });
    dk().args(["--url", &server.base_url(), "admin", "session-idle-timeout"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("predates v0.12.2"));
}

// ---------------------------------------------------------------------------
// knowledge
// ---------------------------------------------------------------------------

#[test]
fn knowledge_full_graph_sends_preview() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/knowledge/graph/full")
            .json_body(json!({"agent_id": "a", "content_preview_chars": 200}));
        then.status(200).json_body(json!({
            "nodes": [], "edges": [], "clusters": [],
            "stats": {"total_memories": 0, "included_memories": 0, "total_edges": 0,
                      "cluster_count": 0, "density": 0.0, "hub_memory_id": null}
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "full-graph",
        "a",
        "--preview",
        "200",
    ])
    .assert()
    .success();
    m.assert();
}
