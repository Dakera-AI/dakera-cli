//! Integration tests for Dakera server v0.12.4 support.
//!
//! Like `v0123.rs`, each test starts a local [`httpmock`] server and runs the
//! compiled `dk` binary against it; no Dakera server is needed. The request
//! bodies and answers follow the engine's request and response types.
//!
//! Covered: `memory forget` by description (`--about`) and by request
//! (`--request`, with the server's reading of it), `--restorable`,
//! `--dry-run`; `memory forget-detect`; `memory trash` / `restore`; the
//! `forget_request` of a store and the `duplicates` / `forget_requests` of a
//! batch store; recall's `instructions`, `associated_memories`, `degraded` and
//! `--include-associated`; `memory get --projection-days`; `session end
//! --auto-summarize` and `summary_pending`; `memory extract`'s `warnings`; a
//! `202` key revocation; a `503` forget refused for want of the relevance
//! model; `namespace policy get` of a namespace without a stored policy.

use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::prelude::*;
use serde_json::{json, Value};

fn dk() -> Command {
    let mut cmd = Command::cargo_bin("dk").expect("dk binary not found — run `cargo build` first");
    cmd.env_remove("DAKERA_API_KEY");
    cmd
}

fn item(id: &str, content: &str, score: f64) -> Value {
    json!({
        "memory": {"id": id, "content": content, "memory_type": "episodic", "importance": 0.5},
        "score": score
    })
}

// ---------------------------------------------------------------------------
// memory forget
// ---------------------------------------------------------------------------

#[test]
fn forget_by_request_sends_it_and_lists_what_was_forgotten() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/forget")
            .json_body(json!({
                "agent_id": "bot",
                "request": "Please forget what I told you about my ex",
                "restorable": true
            }));
        then.status(200).json_body(json!({
            "deleted_count": 2,
            "forgotten_ids": ["m1", "m2"],
            "intent": {"probability": 0.97, "threshold": 0.5, "is_request": true}
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "bot",
        "--request",
        "Please forget what I told you about my ex",
        "--restorable",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("reads as a request to forget"))
    .stdout(predicate::str::contains("Forgot 2 memories of 'bot'"))
    .stdout(predicate::str::contains("dk memory trash bot"))
    .stdout(predicate::str::contains("m2"));
    m.assert();
}

#[test]
fn forget_by_request_that_is_not_one_forgets_nothing() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(200).json_body(json!({
            "deleted_count": 0,
            "forgotten_ids": [],
            "intent": {"probability": 0.03, "threshold": 0.5, "is_request": false}
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "bot",
        "--request",
        "I always forget my keys",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("does not read as a request"))
    .stdout(predicate::str::contains("Nothing was forgotten"));
}

#[test]
fn forget_by_description_dry_run_lists_without_deleting() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/forget")
            .json_body(json!({
                "agent_id": "bot",
                "about": "my home address",
                "min_score": 0.6,
                "limit": 3,
                "dry_run": true
            }));
        then.status(200)
            .json_body(json!({"deleted_count": 0, "forgotten_ids": ["a1"]}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "bot",
        "--about",
        "my home address",
        "--min-score",
        "0.6",
        "--limit",
        "3",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[dry-run] Would forget 1 memory"))
    .stdout(predicate::str::contains("a1"));
    m.assert();
}

#[test]
fn forget_json_prints_the_answer() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(200)
            .json_body(json!({"deleted_count": 1, "forgotten_ids": ["a1"]}));
    });
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "--format",
            "json",
            "memory",
            "forget",
            "bot",
            "--about",
            "sushi",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: Value = serde_json::from_slice(&out).expect("stdout is one JSON document");
    assert_eq!(parsed["forgotten_ids"], json!(["a1"]));
}

#[test]
fn forget_without_the_relevance_model_explains_the_503() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(503)
            .header("Retry-After", "5")
            .json_body(json!({
                "error": "Service unavailable",
                "code": "SERVICE_UNAVAILABLE",
                "status": 503,
                "details": "forget by description needs the relevance model (not loaded); retry"
            }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "bot",
        "--about",
        "sushi",
    ])
    .assert()
    .code(6)
    .stderr(predicate::str::contains("forget by memory id meanwhile"))
    .stderr(predicate::str::contains("retry in 5s"));
}

#[test]
fn forget_with_nothing_to_forget_is_refused_before_sending() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(200).json_body(json!({"deleted_count": 0}));
    });
    dk().args(["--url", &server.base_url(), "memory", "forget", "bot"])
        .assert()
        .code(5);
    m.assert_calls(0);
}

// ---------------------------------------------------------------------------
// memory forget-detect / trash / restore
// ---------------------------------------------------------------------------

#[test]
fn forget_detect_reads_each_message() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/forget/detect")
            .json_body(json!({"texts": ["Oublie mon adresse", "I always forget my keys"]}));
        then.status(200).json_body(json!({
            "model": "multilingual-minilm",
            "threshold": 0.5,
            "sentence_threshold": 0.7,
            "results": [
                {"probability": 0.96, "is_request": true},
                {"probability": 0.04, "is_request": false}
            ]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget-detect",
        "Oublie mon adresse",
        "I always forget my keys",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("1 of 2 message(s)"))
    .stdout(predicate::str::contains("0.960"))
    .stdout(predicate::str::contains("--request"));
    m.assert();
}

#[test]
fn trash_lists_and_restore_puts_back() {
    let server = MockServer::start();
    let list = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/memory/deduplicated")
            .query_param("agent_id", "bot");
        then.status(200).json_body(json!({
            "agent_id": "bot",
            "count": 1,
            "memories": [{
                "id": "m1", "kind": "deletion", "survivor_id": "m1",
                "deleted_at": 1700000000u64, "restorable_until": 1700600000u64,
                "content": "I love sushi"
            }]
        }));
    });
    let restore = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/deduplicated/m1/restore")
            .query_param("agent_id", "bot");
        then.status(200)
            .json_body(json!({"id": "m1", "content": "I love sushi"}));
    });
    dk().args(["--url", &server.base_url(), "memory", "trash", "bot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("I love sushi"))
        .stdout(predicate::str::contains("deletion"));
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "restore",
        "bot",
        "m1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory 'm1' restored"));
    list.assert();
    restore.assert();
}

// ---------------------------------------------------------------------------
// store and batch store
// ---------------------------------------------------------------------------

#[test]
fn store_says_when_the_text_is_a_request_to_forget() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(200).json_body(json!({
            "memory": {"id": "s1", "content": "Please forget my address"},
            "embedding_time_ms": 3,
            "forget_request": {"probability": 0.95, "threshold": 0.5, "is_request": true}
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "bot",
        "Please forget my address",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory stored (id: s1"))
    .stdout(predicate::str::contains("dk memory forget bot --request"));
}

#[test]
fn store_json_prints_the_answer_and_notes_on_stderr() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(200).json_body(json!({
            "memory": {"id": "s1", "content": "x"},
            "embedding_time_ms": 3,
            "forget_request": {"probability": 0.95, "threshold": 0.5, "is_request": true}
        }));
    });
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "-f",
            "json",
            "memory",
            "store",
            "bot",
            "x",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("request to forget"))
        .get_output()
        .stdout
        .clone();
    let parsed: Value = serde_json::from_slice(&out).expect("stdout is one JSON document");
    assert_eq!(parsed["memory"]["id"], "s1");
}

#[test]
fn batch_store_reports_duplicates_and_requests_to_forget() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memories/store/batch");
        then.status(200).json_body(json!({
            "stored": [{"id": "b1", "content": "Forget my job", "importance": 0.5}],
            "stored_count": 1,
            "total_embedding_time_ms": 4,
            "duplicates": [{"index": 1, "existing_id": "old-7"}],
            "forget_requests": [
                {"id": "b1", "probability": 0.9, "threshold": 0.5, "is_request": true}
            ]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-store",
        "bot",
        "-c",
        "Forget my job",
        "-c",
        "Lives in Berlin",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Stored 1 memories"))
    .stdout(predicate::str::contains("#1 = old-7"))
    .stdout(predicate::str::contains("(b1)"));
}

// ---------------------------------------------------------------------------
// recall
// ---------------------------------------------------------------------------

#[test]
fn recall_shows_instructions_associated_and_degraded() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body(json!({
                "agent_id": "bot",
                "query": "library",
                "top_k": 5,
                "min_importance": 0.0,
                "tags": [],
                "neighborhood_mode": "separate",
                "include_associated": true
            }));
        then.status(200).json_body(json!({
            "memories": [item("r1", "uses tokio", 0.9)],
            "instructions": [item("i1", "Always include the version", 0.8)],
            "associated_memories": [item("a1", "tokio is async", 0.4)],
            "degraded": ["graph: read failed"]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "library",
        "--include-associated",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Standing instructions"))
    .stdout(predicate::str::contains("Always include the version"))
    .stdout(predicate::str::contains("Associated"))
    .stdout(predicate::str::contains("tokio is async"))
    .stdout(predicate::str::contains("graph: read failed"));
    m.assert();
}

#[test]
fn recall_can_turn_associated_memories_off() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body_includes(r#"{"include_associated": false}"#);
        then.status(200).json_body(json!({"memories": []}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "q",
        "--include-associated=false",
    ])
    .assert()
    .success();
    m.assert();
}

#[test]
fn recall_json_carries_instructions_and_associated() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200).json_body(json!({
            "memories": [item("r1", "a", 0.9)],
            "instructions": [item("i1", "Answer in French", 0.8)],
            "associated_memories": [item("a1", "b", 0.4)]
        }));
    });
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "-f",
            "json",
            "memory",
            "recall",
            "bot",
            "q",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(parsed["instructions"][0]["id"], "i1");
    assert_eq!(parsed["associated_memories"][0]["id"], "a1");
}

// ---------------------------------------------------------------------------
// get, session end, extract
// ---------------------------------------------------------------------------

#[test]
fn get_with_projection_days_shows_the_projection() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/memory/get/m1")
            .query_param("agent_id", "bot")
            .query_param("projection_days", "2");
        then.status(200).json_body(json!({
            "id": "m1", "content": "x", "importance": 0.8,
            "strength_projection": [
                {"day": 0, "importance": 0.8},
                {"day": 1, "importance": 0.75},
                {"day": 2, "importance": 0.7}
            ]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "get",
        "bot",
        "m1",
        "--projection-days",
        "2",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Projected importance"))
    .stdout(predicate::str::contains("0.75"));
    m.assert();
}

#[test]
fn session_end_sends_auto_summarize_and_warns_of_a_pending_summary() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/sessions/s1/end")
            .json_body(json!({"auto_summarize": true}));
        then.status(200).json_body(json!({
            "session": {"id": "s1", "agent_id": "bot"},
            "memory_count": 5,
            "summary_pending": "the store refused the write"
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "session",
        "end",
        "s1",
        "--auto-summarize",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Session 's1' ended"))
    .stdout(predicate::str::contains("summary memory is not stored yet"));
    m.assert();
}

#[test]
fn extract_shows_the_extractor_and_its_warnings() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memories/extract")
            .json_body(json!({"content": "Mail ana@example.com"}));
        then.status(200).json_body(json!({
            "entities": [{"entity_type": "email", "value": "ana@example.com", "score": 1.0,
                          "start": 5, "end": 20, "tag": "entity:email:ana@example.com"}],
            "count": 1,
            "extractor": "rule_based",
            "warnings": ["the neural pass failed: model not loaded"]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "extract",
        "Mail ana@example.com",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("rules only"))
    .stdout(predicate::str::contains("the neural pass failed"))
    .stdout(predicate::str::contains("ana@example.com"));
}

// ---------------------------------------------------------------------------
// changes the server applied but could not store yet; memory policy
// ---------------------------------------------------------------------------

#[test]
fn key_delete_answered_202_says_it_is_not_stored_yet() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/admin/keys/k1");
        then.status(202).json_body(json!({
            "success": true,
            "message": "API key 'k1' has been deleted on every node now; it could not be stored \
                        yet (timeout), so it is logged on this node and retried until it is"
        }));
    });
    dk().args(["--url", &server.base_url(), "keys", "delete", "k1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not stored yet"))
        .stdout(predicate::str::contains("retried until it is"));
}

#[test]
fn key_delete_refused_prints_no_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/admin/keys/k1");
        then.status(404)
            .json_body(json!({"error": "API key not found", "code": "NOT_FOUND"}));
    });
    dk().args(["--url", &server.base_url(), "keys", "delete", "k1"])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("deleted").not());
}

#[test]
fn policy_get_says_when_nothing_is_stored() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/ns1/memory_policy");
        then.status(200).json_body(json!({
            "working_ttl_seconds": null,
            "episodic_ttl_seconds": null,
            "stored": false
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "policy",
        "get",
        "ns1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("has no stored policy"));
}

#[test]
fn forget_dry_run_is_refused_by_an_older_server_before_anything_is_sent() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(200)
            .json_body(json!({"status": "healthy", "version": "0.12.3"}));
    });
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memory/forget");
        then.status(200).json_body(json!({"deleted_count": 1}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "forget",
        "bot",
        "m1",
        "--dry-run",
    ])
    .assert()
    .code(5)
    .stderr(predicate::str::contains("v0.12.4"))
    .stderr(predicate::str::contains("0.12.3"));
    m.assert_calls(0);
}
