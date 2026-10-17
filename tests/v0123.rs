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

// ---------------------------------------------------------------------------
// dk document (Dakera v0.12.3, Dakera-AI/dakera#923)
// ---------------------------------------------------------------------------

fn scratch_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dk-v0123-doc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn accepted(document_id: &str) -> Value {
    json!({
        "job_id": "job_7_1", "document_id": document_id, "agent_id": "bot",
        "status_url": "/v1/memory/documents/jobs/job_7_1?agent_id=bot"
    })
}

#[test]
fn document_import_markdown_goes_to_the_text_route_and_waits() {
    let server = MockServer::start();
    let file = scratch_file("guide.md", b"# Guide\n\nHello.");
    let import = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/documents")
            .json_body(json!({
                "agent_id": "bot",
                "content": "# Guide\n\nHello.",
                "media_type": "text/markdown",
                "title": "Guide",
                "tags": ["docs", "v1"],
                "chunking": {"max_tokens": 300, "overlap_tokens": 30, "strategy": "fixed"}
            }));
        then.status(202).json_body(accepted("guide"));
    });
    let job = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/memory/documents/jobs/job_7_1")
            .query_param("agent_id", "bot");
        then.status(200).json_body(json!({
            "id": "job_7_1", "job_type": "document_import", "status": "Completed",
            "progress": 100, "message": "document imported: 2 chunks (2 written, 0 unchanged, 0 stale removed); chunks_stored 2/2",
            "metadata": {"document_id": "guide"}
        }));
    });
    dk().args(["--url", &server.base_url(), "document", "import"])
        .arg(&file)
        .args([
            "--agent",
            "bot",
            "--title",
            "Guide",
            "--tag",
            "docs",
            "--tag",
            "v1",
            "--chunk-tokens",
            "300",
            "--overlap",
            "30",
            "--strategy",
            "fixed",
            "--wait",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Document 'guide' imported"))
        .stdout(predicate::str::contains("2 chunks"));
    import.assert();
    job.assert();
}

#[test]
fn document_import_pdf_uploads_then_imports_the_attachment() {
    let server = MockServer::start();
    let file = scratch_file("manual.pdf", b"%PDF-1.7 test");
    let upload = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/_dakera_agent_bot/attachments")
            .header("content-type", "application/pdf")
            .body("%PDF-1.7 test");
        then.status(201)
            .json_body(json!({"attachment_ref": "sha256:ab12", "size_bytes": 13}));
    });
    let import = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/_dakera_agent_bot/attachments/sha256:ab12/document")
            .json_body(json!({"agent_id": "bot", "document_id": "manual-v3", "decompose": true}));
        then.status(202).json_body(accepted("manual-v3"));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "-f",
        "json",
        "document",
        "import",
    ])
    .arg(&file)
    .args([
        "--agent",
        "bot",
        "--document-id",
        "manual-v3",
        "--decompose",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"job_id\": \"job_7_1\""));
    upload.assert();
    import.assert();
}

#[test]
fn document_import_needs_a_known_format() {
    let file = scratch_file("data.bin", b"\x00\x01");
    dk().args(["--url", "http://127.0.0.1:1", "document", "import"])
        .arg(&file)
        .args(["--agent", "bot"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("--media-type"));
}

#[test]
fn document_import_reports_a_failed_job() {
    let server = MockServer::start();
    let file = scratch_file("bad.html", b"<html>");
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/documents");
        then.status(202).json_body(accepted("bad"));
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/documents/jobs/job_7_1");
        then.status(200).json_body(json!({
            "id": "job_7_1", "status": "Failed", "progress": 2,
            "message": "HTML nested deeper than 512 block elements",
            "metadata": {}, "error": {"status": 422, "code": "UNPROCESSABLE_CONTENT"}
        }));
    });
    dk().args(["--url", &server.base_url(), "document", "import"])
        .arg(&file)
        .args(["--agent", "bot", "--wait"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("422 UNPROCESSABLE_CONTENT"));
}

#[test]
fn document_routes_on_an_older_server_say_what_is_needed() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/documents");
        then.status(404).json_body(json!({
            "error": "No route for GET /v1/memory/documents", "code": "ROUTE_NOT_FOUND", "status": 404
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "document",
        "list",
        "--agent",
        "bot",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("needs a Dakera v0.12.3 server"));
}

#[test]
fn document_list_show_job_and_delete() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/memory/documents")
            .query_param("agent_id", "bot")
            .query_param("limit", "1");
        then.status(200).json_body(json!({
            "documents": [{
                "document_id": "manual-v3", "title": "Field Manual", "media_type": "application/pdf",
                "chunk_count": 212, "bytes": 1843311, "pages": 212, "created_at": 1, "updated_at": 2,
                "sha256": "ab"
            }],
            "next_cursor": "manual-v3"
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "document",
        "list",
        "--agent",
        "bot",
        "--limit",
        "1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Field Manual"))
    .stdout(predicate::str::contains("--cursor manual-v3"));

    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/documents/manual-v3");
        then.status(200).json_body(json!({
            "document": {"document_id": "manual-v3", "title": "Field Manual",
                         "media_type": "application/pdf", "chunk_count": 1, "bytes": 9,
                         "created_at": 1, "updated_at": 2, "sha256": "ab"},
            "chunks": [{"id": "doc_x", "chunk_index": 0, "page_start": 3, "page_end": 4,
                        "heading_path": ["Field Manual", "Install"], "char_start": 0, "char_end": 80}]
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "document",
        "show",
        "manual-v3",
        "--agent",
        "bot",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Field Manual › Install"))
    .stdout(predicate::str::contains("3-4"));

    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/documents/jobs/job_9");
        then.status(200).json_body(json!({
            "id": "job_9", "status": "Running", "progress": 40,
            "message": "chunks_stored 32/80", "metadata": {"document_id": "manual-v3"}
        }));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "document",
        "job",
        "job_9",
        "--agent",
        "bot",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Running"))
    .stdout(predicate::str::contains("40%"));

    let del = server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/memory/documents/manual-v3")
            .query_param("agent_id", "bot");
        then.status(200)
            .json_body(json!({"document_id": "manual-v3", "deleted_chunks": 212}));
    });
    dk().args([
        "--url",
        &server.base_url(),
        "document",
        "delete",
        "manual-v3",
        "--agent",
        "bot",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("212 chunk memories"));
    del.assert();
}

#[test]
fn recall_cites_document_chunks() {
    let server = MockServer::start();
    let mut chunk = item("doc_1", "Field Manual › Install\n\nRun the installer.", 0.9);
    chunk["memory"]["metadata"] = json!({"_dakera_document": {
        "id": "manual-v3", "title": "Field Manual", "media_type": "application/pdf",
        "chunk_index": 4, "chunk_count": 9, "page_start": 39, "page_end": 40,
        "heading_path": ["Field Manual", "Install"], "char_start": 0, "char_end": 9, "sha256": "ab"
    }});
    let answer = json!({"memories": [chunk, item("m2", "unrelated", 0.4)]});
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/recall");
        then.status(200).json_body(answer);
    });
    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "install",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("SOURCE"))
    .stdout(predicate::str::contains("Field Manual, p. 39-40 › Install"));
    let out = dk()
        .args([
            "--url",
            &server.base_url(),
            "-f",
            "json",
            "memory",
            "recall",
            "bot",
            "install",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(doc["memories"][0]["document"]["document_id"], "manual-v3");
    assert_eq!(doc["memories"][0]["document"]["page_start"], 39);
    assert!(doc["memories"][1].get("document").is_none());
}
