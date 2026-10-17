//! Integration tests for the recall and hardening changes that follow the
//! Dakera engine's recall API correctness work (Dakera-AI/dakera#917) and
//! security hardening (Dakera-AI/dakera#920).
//!
//! Like `v012.rs`, each test starts a local [`httpmock`] server and runs the
//! compiled `dk` binary against it; no Dakera server is needed.
//!
//! Covered: `memory recall` asking for `neighborhood_mode` (default
//! `separate`) and `include_summaries`, the ranked table honouring `--top-k`
//! with the neighbourhood in its own Context section, `matched_text`, the JSON
//! output, an older server that appends its context inline; the `422` for an
//! unknown admin field; `config show` never printing the key; the config file
//! permissions warning.

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
// memory recall
// ---------------------------------------------------------------------------

#[test]
fn recall_asks_for_separate_context_by_default() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body(json!({
                "agent_id": "bot",
                "query": "port",
                "top_k": 2,
                "min_importance": 0.0,
                "tags": [],
                "neighborhood_mode": "separate"
            }));
        then.status(200).json_body(json!({"memories": []}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "port",
        "-k",
        "2",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
    m.assert();
}

#[test]
fn recall_off_and_include_summaries_are_sent() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body_includes(r#"{"neighborhood_mode": "off", "neighborhood": false}"#)
            .json_body_includes(r#"{"include_summaries": false}"#);
        then.status(200).json_body(json!({"memories": []}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "q",
        "--neighborhood",
        "off",
        "--include-summaries=false",
    ])
    .assert()
    .success();
    m.assert();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body_includes(r#"{"include_summaries": true}"#)
            .json_body_includes(r#"{"neighborhood_mode": "inline"}"#);
        then.status(200).json_body(json!({"memories": []}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "q",
        "--neighborhood",
        "inline",
        "--include-summaries",
    ])
    .assert()
    .success();
    m.assert();
}

#[test]
fn recall_rejects_an_unknown_neighborhood_mode() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "memory",
        "recall",
        "bot",
        "q",
        "--neighborhood",
        "sideways",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("separate"));
}

fn separate_answer() -> Value {
    let mut collapsed = item("m1", "Deploy notes for the API", 0.9);
    collapsed["matched_text"] = json!("The API now listens on port 9000.");
    collapsed["matched_ids"] = json!(["m1-s3"]);
    let mut neighbour = item("n1", "We moved it last Tuesday.", 0.41);
    neighbour["retrieval"] = json!("neighborhood");
    neighbour["context_of"] = json!("m2");
    json!({
        "memories": [collapsed, item("m2", "Port change approved", 0.8)],
        "context_memories": [neighbour],
        "query_embedding_time_ms": 1,
        "search_time_ms": 2
    })
}

#[test]
fn recall_table_shows_ranked_results_then_a_context_section() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200).json_body(separate_answer());
    });
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "memory",
            "recall",
            "bot",
            "port",
            "-k",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("Found 2 memories"), "{out}");
    assert!(out.contains("MATCHED_TEXT"), "{out}");
    assert!(out.contains("port 9000"), "{out}");
    let context_at = out.find("Context: 1 session neighbour").expect(&out);
    let neighbour_at = out.find("We moved it last Tuesday.").expect(&out);
    assert!(neighbour_at > context_at, "context printed apart: {out}");
    assert!(out.contains("#2 m2"), "context_of with its rank: {out}");
}

#[test]
fn recall_json_is_one_document_with_both_lists() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200).json_body(separate_answer());
    });
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "--format",
            "json",
            "memory",
            "recall",
            "bot",
            "port",
            "-k",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: Value = serde_json::from_slice(&out).expect("stdout is JSON only");
    assert_eq!(doc["memories"].as_array().unwrap().len(), 2);
    assert_eq!(
        doc["memories"][0]["matched_text"],
        "The API now listens on port 9000."
    );
    assert_eq!(doc["memories"][0]["matched_ids"], json!(["m1-s3"]));
    assert!(doc["memories"][1].get("matched_text").is_none());
    assert_eq!(doc["context_memories"][0]["id"], "n1");
    assert_eq!(doc["context_memories"][0]["context_of"], "m2");
    assert_eq!(doc["context_memories"][0]["context_of_rank"], 2);
}

#[test]
fn recall_from_an_older_server_keeps_to_top_k() {
    // A v0.12.2 server ignores neighborhood_mode and appends its context.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200).json_body(json!({
            "memories": [
                item("m1", "first", 0.9),
                item("m2", "second", 0.8),
                item("n1", "neighbour one", 0.3),
                item("n2", "neighbour two", 0.2)
            ]
        }));
    });
    let assert = dk()
        .args([
            "--url",
            &server.base_url(),
            "--format",
            "json",
            "memory",
            "recall",
            "bot",
            "q",
            "-k",
            "2",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "2 neighbourhood context item(s) left out",
        ));
    let doc: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    let ids: Vec<&str> = doc["memories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["m1", "m2"]);
    assert_eq!(doc["context_memories"], json!([]));

    // Asked for inline, the overflow is the context the user wanted.
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "q",
        "-k",
        "2",
        "--neighborhood",
        "inline",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Found 2 memories"))
    .stdout(predicate::str::contains("Context: 2 session neighbour"))
    .stdout(predicate::str::contains("neighbour two"));
}

// ---------------------------------------------------------------------------
// Strict admin routes and the key
// ---------------------------------------------------------------------------

#[test]
fn key_create_sends_only_known_fields_and_explains_a_422() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/admin/keys").json_body(json!({
            "name": "ci", "scope": "read", "expires_in_days": 7
        }));
        then.status(422).json_body(json!({
            "error": "Failed to deserialize the JSON body into the target type: unknown field \
                      `expires_in_days`, expected one of `name`, `scope`",
            "code": "INVALID_REQUEST",
            "status": 422
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "keys",
        "create",
        "ci",
        "--expires",
        "7",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "does not accept the field `expires_in_days`",
    ));
    m.assert();
}

#[test]
fn key_ids_are_encoded_in_paths() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/admin/keys/a%2Fb");
        then.status(200).json_body(json!({"key_id": "a/b"}));
    });
    dk().args(["--url", &server.base_url(), "keys", "get", "a/b"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn config_show_masks_the_key() {
    let home = std::env::temp_dir().join(format!("dk-v0123-home-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let secret = "dk_live_0123456789abcdefSECRET9876";
    let assert = dk()
        .env("HOME", &home)
        .env("DAKERA_API_KEY", secret)
        .env("DAKERA_URL", "https://dakera.example.com")
        .args(["config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dk_…9876"))
        .stdout(predicate::str::contains("TLS (https)"));
    let out = assert.get_output();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!all.contains("0123456789abcdef"), "{all}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[cfg(unix)]
#[test]
fn profile_add_writes_an_owner_only_file_and_a_readable_one_is_flagged() {
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir().join(format!("dk-v0123-perm-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    dk().env("HOME", &home)
        .args([
            "config",
            "profile",
            "add",
            "prod",
            "--url",
            "https://d.example.com",
        ])
        .assert()
        .success();
    let path = home.join(".dakera").join("config.toml");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    dk().env("HOME", &home)
        .args(["config"])
        .assert()
        .success()
        .stderr(predicate::str::contains("readable by other users"));
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_key_over_plain_http_to_a_remote_host_is_warned_about() {
    dk().env("DAKERA_API_KEY", "dk_live_abcdefghijklmnop")
        .args(["--url", "http://192.0.2.1:9", "config"])
        .assert()
        .success()
        .stderr(predicate::str::contains("sent unencrypted"));
}
