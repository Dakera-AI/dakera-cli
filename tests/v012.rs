//! Integration tests for the Dakera v0.12 commands of `dk`.
//!
//! Like `integration.rs`, each test starts a local [`httpmock`] server and runs
//! the compiled `dk` binary against it; no Dakera server is needed.
//!
//! Covered: `health ready|live|--detailed`, `capabilities`, `attachment *`,
//! `admin embed-migration|encryption-*|backup-*|quotas-set`, and how the
//! v0.12 error bodies (`403` pinned key / missing `super_admin`, `413`, `501`,
//! `503` with `Retry-After`) become messages and exit codes.

use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::prelude::*;
use serde_json::json;

fn dk() -> Command {
    let mut cmd = Command::cargo_bin("dk").expect("dk binary not found — run `cargo build` first");
    cmd.env_remove("DAKERA_API_KEY");
    cmd
}

/// A scratch file path unique to this test process.
fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("dk-v012-{}-{name}", std::process::id()))
}

// ---------------------------------------------------------------------------
// health
// ---------------------------------------------------------------------------

#[test]
fn health_ready_reports_ready_with_checks() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health/ready");
        then.status(200).json_body(json!({
            "ready": true,
            "version": "0.12.0",
            "checks": {
                "storage": {"status": "ok", "message": null},
                "embedding_engine": {"status": "ok", "message": null},
                "tiered_engine": {"status": "disabled", "message": null}
            }
        }));
    });

    dk().args(["--url", &server.base_url(), "health", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is ready"))
        .stdout(predicate::str::contains("storage: ok"))
        .stdout(predicate::str::contains("tiered_engine: disabled"));
}

#[test]
fn health_ready_while_starting_exits_6_and_shows_retry_after() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health/ready");
        then.status(503)
            .header("Retry-After", "5")
            .json_body(json!({
                "ready": false,
                "version": "0.12.0",
                "starting": true,
                "reason": "downloading models",
                "downloads": [
                    {"repo": "BAAI/bge-large", "file": "model.onnx", "received_bytes": 10, "total_bytes": 100}
                ]
            }));
    });

    dk().args(["--url", &server.base_url(), "health", "ready"])
        .assert()
        .failure()
        .code(6)
        .stdout(predicate::str::contains("Reason: downloading models"))
        .stdout(predicate::str::contains(
            "Downloading BAAI/bge-large/model.onnx: 10 / 100",
        ))
        .stderr(predicate::str::contains("retry in 5s"));
}

#[test]
fn health_ready_json_error_carries_retry_after() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health/ready");
        then.status(503)
            .header("Retry-After", "7")
            .json_body(json!({"ready": false, "version": "0.12.0", "checks": {}}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "health",
        "ready",
    ])
    .assert()
    .failure()
    .code(6)
    .stderr(predicate::str::contains("\"retry_after_secs\": 7"))
    .stderr(predicate::str::contains("\"http_status\": 503"));
}

#[test]
fn health_live_prints_uptime() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health/live");
        then.status(200)
            .json_body(json!({"alive": true, "version": "0.12.0", "uptime_seconds": 3700}));
    });

    dk().args(["--url", &server.base_url(), "health", "live"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is alive"))
        .stdout(predicate::str::contains("Uptime: 1h 1m"));
}

#[test]
fn health_degraded_exits_0_and_lists_components_and_migration() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(200).json_body(json!({
            "service": "dakera",
            "status": "degraded",
            "version": "0.12.0",
            "unreadable_records": 0,
            "corrupt_records": 0,
            "advice": "components are running degraded",
            "degraded": [{"component": "reranker", "reason": "model missing"}],
            "config_warnings": [{"component": "env_name", "reason": "DAKERA_X is ignored"}],
            "embed_migration": {
                "state": "running", "remaining": 50, "reembedded": 10, "skipped": 0, "eta_secs": 90
            }
        }));
    });

    dk().args(["--url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("degraded"))
        .stdout(predicate::str::contains("reranker: model missing"))
        .stdout(predicate::str::contains("env_name: DAKERA_X is ignored"))
        .stdout(predicate::str::contains(
            "Embed migration: running (50 remaining",
        ));
}

#[test]
fn health_while_starting_exits_6() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(503)
            .header("Retry-After", "5")
            .json_body(json!({
                "service": "dakera", "status": "starting", "version": "0.12.0",
                "reason": "loading models", "downloads": []
            }));
    });

    dk().args(["--url", &server.base_url(), "health"])
        .assert()
        .failure()
        .code(6)
        .stderr(predicate::str::contains("loading models"))
        .stderr(predicate::str::contains("retry in 5s"));
}

#[test]
fn health_json_format_prints_the_servers_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(200)
            .json_body(json!({"service": "dakera", "status": "healthy", "version": "0.12.0"}));
    });

    dk().args(["--url", &server.base_url(), "--format", "json", "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"healthy\""));
}

#[test]
fn health_detailed_combines_the_probes() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/health");
        then.status(200).json_body(json!({
            "service": "dakera", "status": "healthy", "version": "0.12.0", "build_sha": "abc123"
        }));
    });
    server.mock(|when, then| {
        when.method(GET).path("/health/ready");
        then.status(200)
            .json_body(json!({"ready": true, "version": "0.12.0", "checks": {}}));
    });
    server.mock(|when, then| {
        when.method(GET).path("/health/live");
        then.status(200)
            .json_body(json!({"alive": true, "version": "0.12.0", "uptime_seconds": 90}));
    });
    server.mock(|when, then| {
        when.method(GET).path("/ops/diagnostics");
        then.status(200).json_body(json!({
            "resources": {"memory_bytes": 104857600, "thread_count": 12, "open_fds": 30},
            "active_jobs": 2
        }));
    });

    dk().args(["--url", &server.base_url(), "health", "--detailed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Healthy"))
        .stdout(predicate::str::contains("1m 30s"))
        .stdout(predicate::str::contains("abc123"))
        .stdout(predicate::str::contains("Memory Used: 100 MB"))
        .stdout(predicate::str::contains("Active Jobs: 2"));
}

#[test]
fn health_ready_unreachable_server_exits_2() {
    dk().args(["--url", "http://127.0.0.1:1", "health", "ready"])
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// capabilities
// ---------------------------------------------------------------------------

fn capabilities_body() -> serde_json::Value {
    json!({
        "capabilities_version": 1,
        "server_version": "0.12.0",
        "api_versions": ["v1"],
        "default_model": "bge-large",
        "search_mode": "hnsw",
        "scoring": {"strategy": "single-vector"},
        "attachments": {
            "enabled": false,
            "max_bytes": 26214400,
            "transcription": {"model": "whisper-tiny.en"}
        },
        "vision": {"enabled": false},
        "records": {"enabled": true},
        "query_languages": ["en", "de", "fr"],
        "reembed_pending": false,
        "unreadable_records": 0,
        "on_disk_format_version": 3
    })
}

#[test]
fn capabilities_table_shows_models_and_feature_switches() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/capabilities");
        then.status(200).json_body(capabilities_body());
    });

    dk().args(["--url", &server.base_url(), "capabilities"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0.12.0"))
        .stdout(predicate::str::contains("bge-large"))
        .stdout(predicate::str::contains("disabled (set DAKERA_ATTACHMENTS"))
        .stdout(predicate::str::contains("en, de, fr"));
}

#[test]
fn capabilities_json_prints_the_document() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/capabilities");
        then.status(200).json_body(capabilities_body());
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "capabilities",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"capabilities_version\": 1"));
}

#[test]
fn capabilities_on_a_v011_server_exits_3_and_says_why() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/capabilities");
        then.status(404);
    });

    dk().args(["--url", &server.base_url(), "capabilities"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("predates Dakera v0.12"));
}

// ---------------------------------------------------------------------------
// attachments
// ---------------------------------------------------------------------------

#[test]
fn attachment_upload_sends_the_bytes_with_the_guessed_media_type() {
    let server = MockServer::start();
    let file = scratch("note.wav");
    std::fs::write(&file, b"RIFFfake").unwrap();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/uploads/attachments")
            .header("Content-Type", "audio/wav")
            .body("RIFFfake");
        then.status(201).json_body(json!({
            "attachment_ref": "sha256:abc", "content_type": "audio/wav",
            "size_bytes": 8, "created": true
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "upload",
        "uploads",
    ])
    .arg(&file)
    .assert()
    .success()
    .stdout(predicate::str::contains("sha256:abc"));
    m.assert();
    let _ = std::fs::remove_file(&file);
}

#[test]
fn attachment_upload_over_the_limit_exits_5_with_the_variable_named() {
    let server = MockServer::start();
    let file = scratch("big.bin");
    std::fs::write(&file, b"x").unwrap();
    server.mock(|when, then| {
        when.method(POST).path("/v1/namespaces/uploads/attachments");
        then.status(413).json_body(json!({
            "error": "the request body is larger than DAKERA_ATTACHMENT_MAX_BYTES (26214400 bytes)",
            "code": "PAYLOAD_TOO_LARGE", "status": 413
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "upload",
        "uploads",
    ])
    .arg(&file)
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("PAYLOAD_TOO_LARGE"))
    .stderr(predicate::str::contains("DAKERA_ATTACHMENT_MAX_BYTES"));
    let _ = std::fs::remove_file(&file);
}

#[test]
fn attachment_upload_missing_file_exits_5_without_a_request() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "attachment",
        "upload",
        "uploads",
        "/nonexistent/x.wav",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("failed to read file"));
}

#[test]
fn attachment_routes_off_exit_6_and_name_the_switch() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/uploads/attachments");
        then.status(501).json_body(json!({
            "error": "attachments are disabled on this server",
            "code": "FEATURE_DISABLED", "status": 501,
            "details": "set DAKERA_ATTACHMENTS=1 to turn them on"
        }));
    });

    dk().args(["--url", &server.base_url(), "attachment", "list", "uploads"])
        .assert()
        .failure()
        .code(6)
        .stderr(predicate::str::contains("FEATURE_DISABLED"))
        .stderr(predicate::str::contains("DAKERA_ATTACHMENTS=1"))
        .stderr(predicate::str::contains("switched off"));
}

#[test]
fn attachment_list_shows_the_entries() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/uploads/attachments");
        then.status(200).json_body(json!({"attachments": [
            {"attachment_ref": "sha256:abc", "content_type": "audio/wav", "size_bytes": 8}
        ]}));
    });

    dk().args(["--url", &server.base_url(), "attachment", "list", "uploads"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sha256:abc"))
        .stdout(predicate::str::contains("audio/wav"));
}

#[test]
fn attachment_list_empty_says_so() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/uploads/attachments");
        then.status(200).json_body(json!({"attachments": []}));
    });

    dk().args(["--url", &server.base_url(), "attachment", "list", "uploads"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No attachments"));
}

#[test]
fn attachment_download_writes_the_bytes() {
    let server = MockServer::start();
    let out = scratch("download.wav");
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/namespaces/uploads/attachments/sha256:abc");
        then.status(200)
            .header("Content-Type", "audio/wav")
            .body("RIFFfake");
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "download",
        "uploads",
        "sha256:abc",
        "-o",
    ])
    .arg(&out)
    .assert()
    .success()
    .stdout(predicate::str::contains("Saved 8 bytes"));
    assert_eq!(std::fs::read(&out).unwrap(), b"RIFFfake");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn attachment_delete_succeeds_on_204() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/namespaces/uploads/attachments/sha256:abc");
        then.status(204);
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "delete",
        "uploads",
        "sha256:abc",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("deleted"));
}

#[test]
fn attachment_delete_while_referenced_exits_5() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/namespaces/uploads/attachments/sha256:abc");
        then.status(409).json_body(json!({
            "error": "attachment is referenced by 2 memories", "code": "CONFLICT", "status": 409
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "delete",
        "uploads",
        "sha256:abc",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("referenced by 2 memories"));
}

fn accepted_body() -> serde_json::Value {
    json!({
        "job_id": "job_1_0",
        "attachment_ref": "sha256:abc",
        "agent_id": "bot",
        "memory_id": "mem_1",
        "model": "whisper-tiny.en",
        "status_url": "/v1/namespaces/uploads/attachments/sha256:abc/transcribe/job_1_0"
    })
}

#[test]
fn attachment_transcribe_sends_the_memory_fields_and_prints_the_job() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/transcribe")
            .json_body(json!({"agent_id": "bot", "tags": ["voice"], "lang": "en"}));
        then.status(202).json_body(accepted_body());
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "transcribe",
        "uploads",
        "sha256:abc",
        "--agent-id",
        "bot",
        "--tag",
        "voice",
        "--lang",
        "en",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("job_1_0"));
    m.assert();
}

#[test]
fn attachment_transcribe_wait_polls_until_completed() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/transcribe");
        then.status(202).json_body(accepted_body());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/transcribe/job_1_0");
        then.status(200).json_body(json!({
            "id": "job_1_0", "status": "Completed", "progress": 100,
            "message": "memory mem_1 stored"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "transcribe",
        "uploads",
        "sha256:abc",
        "--agent-id",
        "bot",
        "--wait",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("memory mem_1 stored"));
}

#[test]
fn attachment_transcribe_wait_failed_job_exits_5() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/transcribe");
        then.status(202).json_body(accepted_body());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/transcribe/job_1_0");
        then.status(200).json_body(json!({
            "id": "job_1_0", "status": "Failed", "progress": 5,
            "message": "the audio holds no speech",
            "error": {"status": 400, "code": "INVALID_REQUEST"}
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "transcribe",
        "uploads",
        "sha256:abc",
        "--agent-id",
        "bot",
        "--wait",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("holds no speech"))
    .stderr(predicate::str::contains("INVALID_REQUEST"));
}

#[test]
fn attachment_index_sends_the_caption() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/index")
            .json_body(json!({"agent_id": "bot", "content": "page 3"}));
        then.status(202).json_body(json!({
            "job_id": "job_2_0", "attachment_ref": "sha256:abc", "agent_id": "bot",
            "memory_id": "mem_2", "model": "colmodernvbert",
            "status_url": "/v1/namespaces/uploads/attachments/sha256:abc/index/job_2_0"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "index",
        "uploads",
        "sha256:abc",
        "--agent-id",
        "bot",
        "--content",
        "page 3",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("job_2_0"));
    m.assert();
}

#[test]
fn attachment_job_reads_the_status_route_of_its_kind() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/namespaces/uploads/attachments/sha256:abc/index/job_2_0");
        then.status(200).json_body(json!({
            "id": "job_2_0", "status": "Running", "progress": 10, "message": "embedding"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "attachment",
        "job",
        "uploads",
        "sha256:abc",
        "job_2_0",
        "--kind",
        "index",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Running"));
}

// ---------------------------------------------------------------------------
// admin: permissions and error bodies
// ---------------------------------------------------------------------------

#[test]
fn namespace_pinned_key_on_a_node_wide_route_exits_4_and_explains() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/encryption/status");
        then.status(403).json_body(json!({
            "error": "Access denied to namespace", "code": "NAMESPACE_ACCESS_DENIED",
            "status": 403, "details": "namespace: *"
        }));
    });

    dk().args(["--url", &server.base_url(), "admin", "encryption-status"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("NAMESPACE_ACCESS_DENIED"))
        .stderr(predicate::str::contains("pinned to namespaces"));
}

#[test]
fn backup_download_with_an_admin_key_exits_4_and_asks_for_super_admin() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/b1/download");
        then.status(403).json_body(json!({
            "error": "Insufficient scope for this operation", "code": "INSUFFICIENT_SCOPE",
            "status": 403, "details": "required: super_admin, actual: admin"
        }));
    });
    let out = scratch("denied.json.gz");

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-download",
        "b1",
        "-o",
    ])
    .arg(&out)
    .assert()
    .failure()
    .code(4)
    .stderr(predicate::str::contains("required: super_admin"))
    .stderr(predicate::str::contains("global super_admin key"));
    assert!(!out.exists(), "nothing must be written on a refusal");
}

#[test]
fn json_error_output_carries_the_servers_fields() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/reembed/migration");
        then.status(403).json_body(json!({
            "error": "Access denied to namespace", "code": "NAMESPACE_ACCESS_DENIED",
            "status": 403, "details": "namespace: *"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "admin",
        "embed-migration",
    ])
    .assert()
    .failure()
    .code(4)
    .stderr(predicate::str::contains("\"http_status\": 403"))
    .stderr(predicate::str::contains(
        "\"server_code\": \"NAMESPACE_ACCESS_DENIED\"",
    ))
    .stderr(predicate::str::contains("\"details\": \"namespace: *\""));
}

#[test]
fn a_failed_admin_call_does_not_print_a_success_line() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/admin/namespaces/docs/optimize");
        then.status(403).json_body(json!({
            "error": "Insufficient scope for this operation", "code": "INSUFFICIENT_SCOPE",
            "details": "required: admin, actual: read"
        }));
    });

    dk().args(["--url", &server.base_url(), "admin", "optimize", "docs"])
        .assert()
        .failure()
        .code(4)
        .stdout(predicate::str::contains("optimization started").not());
}

// ---------------------------------------------------------------------------
// admin: embed migration, encryption
// ---------------------------------------------------------------------------

#[test]
fn embed_migration_shows_progress_and_namespaces() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/reembed/migration");
        then.status(200)
            .json_body(json!({
                "state": "running", "remaining": 120, "reembedded": 30, "skipped": 1,
                "rate_per_sec": 4.5, "eta_secs": 27,
                "target": {"side": "document", "model": "bge-large", "recipe": "r1"},
                "rate_limit_per_sec": 20.0,
                "cursor": {},
                "namespaces": {
                    "_dakera_agent_bot": {"remaining": 120, "reembedded": 30, "skipped": 1, "done": false}
                }
            }));
    });

    dk().args(["--url", &server.base_url(), "admin", "embed-migration"])
        .assert()
        .success()
        .stdout(predicate::str::contains("running (120 remaining, 30 re"))
        .stdout(predicate::str::contains("bge-large"))
        .stdout(predicate::str::contains("_dakera_agent_bot"));
}

#[test]
fn encryption_status_lists_keys_and_namespace_keys() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/encryption/status");
        then.status(200)
            .json_body(json!({
                "enabled": true, "node_id": "node-1",
                "default_key_id": "k2", "environment_key_id": "k1",
                "namespace_keys": {"team-a": "k3"},
                "keys": [
                    {"key_id": "k1", "origin": "environment", "created_at_ms": 1, "active_for": [],
                     "retired_at_ms": 5, "referenced_on_this_node": 0},
                    {"key_id": "k3", "origin": "generated", "created_at_ms": 9, "active_for": ["team-a"],
                     "retired_at_ms": null, "referenced_on_this_node": 42}
                ],
                "reseal": {"current": null, "last": null, "passes_completed": 3, "retry_pending": false},
                "nodes": []
            }));
    });

    dk().args(["--url", &server.base_url(), "admin", "encryption-status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("node-1"))
        .stdout(predicate::str::contains("k3"))
        .stdout(predicate::str::contains("team-a"));
}

#[test]
fn encryption_status_when_not_configured_says_so() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/encryption/status");
        then.status(200)
            .json_body(json!({
                "enabled": false, "node_id": "node-1", "namespace_keys": {}, "keys": [],
                "reseal": {"current": null, "last": null, "passes_completed": 0, "retry_pending": false},
                "nodes": []
            }));
    });

    dk().args(["--url", &server.base_url(), "admin", "encryption-status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not configured"));
}

#[test]
fn encryption_rotate_one_namespace() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/encryption/rotate-key")
            .json_body(json!({"namespace": "team-a", "wait_secs": 0}));
        then.status(200).json_body(json!({
            "key_id": "k4", "previous_key_id": "k3", "scope": "namespace", "namespace": "team-a",
            "reseal": "running", "rotated": 0, "skipped": 0, "namespaces": [],
            "fulltext_indices_rotated": 0, "failed_namespaces": [],
            "status_url": "/admin/encryption/status"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "encryption-rotate",
        "-n",
        "team-a",
        "--wait-secs",
        "0",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("namespace 'team-a'"))
    .stdout(predicate::str::contains("k4"))
    .stdout(predicate::str::contains("continues in the background"));
    m.assert();
}

#[test]
fn encryption_rotate_reads_the_new_key_from_the_environment() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/encryption/rotate-key")
            .json_body(json!({"new_key": "a-long-enough-passphrase"}));
        then.status(200).json_body(json!({
            "key_id": "k5", "previous_key_id": "k4", "scope": "global", "namespace": null,
            "reseal": "completed", "rotated": 12, "skipped": 0, "namespaces": ["a"],
            "fulltext_indices_rotated": 1, "failed_namespaces": [],
            "status_url": "/admin/encryption/status"
        }));
    });

    dk().env("DK_TEST_NEW_KEY", "a-long-enough-passphrase")
        .args([
            "--url",
            &server.base_url(),
            "admin",
            "encryption-rotate",
            "--new-key-env",
            "DK_TEST_NEW_KEY",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("global encryption key"))
        .stdout(predicate::str::contains("12"));
    m.assert();
}

#[test]
fn encryption_rotate_with_an_unset_key_variable_exits_5() {
    dk().env_remove("DK_TEST_UNSET_KEY")
        .args([
            "--url",
            "http://127.0.0.1:1",
            "admin",
            "encryption-rotate",
            "--new-key-env",
            "DK_TEST_UNSET_KEY",
        ])
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("DK_TEST_UNSET_KEY"));
}

#[test]
fn encryption_reseal_posts_the_namespace() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/encryption/reseal")
            .json_body(json!({"namespace": "team-a"}));
        then.status(202)
            .json_body(json!({"reseal": "running", "status_url": "/admin/encryption/status"}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "encryption-reseal",
        "-n",
        "team-a",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Re-seal pass started"));
    m.assert();
}

// ---------------------------------------------------------------------------
// admin: backups
// ---------------------------------------------------------------------------

#[test]
fn backup_create_sends_the_required_name() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/admin/backups").json_body(json!({
            "name": "nightly", "backup_type": "full", "namespaces": ["a", "b"],
            "encrypt": true, "compression": "zstd"
        }));
        then.status(202).json_body(json!({"backup": {
            "backup_id": "b1", "name": "nightly", "status": "inprogress"
        }}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-create",
        "--name",
        "nightly",
        "--type",
        "full",
        "-n",
        "a",
        "-n",
        "b",
        "--encrypt",
        "--compression",
        "zstd",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Backup 'b1' started"));
    m.assert();
}

#[test]
fn backup_create_wait_polls_until_completed() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/admin/backups");
        then.status(202).json_body(json!({"backup": {
            "backup_id": "b1", "name": "x", "status": "inprogress"
        }}));
    });
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/b1");
        then.status(200).json_body(json!({
            "backup_id": "b1", "name": "x", "status": "completed", "size_bytes": 1024
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-create",
        "--name",
        "x",
        "--wait",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("completed"));
}

#[test]
fn backup_create_wait_failed_backup_exits_nonzero_with_the_reason() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/admin/backups");
        then.status(202)
            .json_body(json!({"backup": {"backup_id": "b1", "status": "inprogress"}}));
    });
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/b1");
        then.status(200).json_body(json!({
            "backup_id": "b1", "status": "failed", "error": "S3 bucket unreachable"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-create",
        "--name",
        "x",
        "--wait",
    ])
    .assert()
    .failure()
    .code(6)
    .stderr(predicate::str::contains("S3 bucket unreachable"));
}

#[test]
fn backup_download_writes_the_bundle() {
    let server = MockServer::start();
    let out = scratch("bundle.json.gz");
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/b1/download");
        then.status(200)
            .header("Content-Type", "application/gzip")
            .body("GZIPBYTES");
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-download",
        "b1",
        "-o",
    ])
    .arg(&out)
    .assert()
    .success()
    .stdout(predicate::str::contains("Saved 9 bytes"));
    assert_eq!(std::fs::read(&out).unwrap(), b"GZIPBYTES");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn backup_upload_sends_a_gzip_bundle_as_gzip() {
    let server = MockServer::start();
    let file = scratch("upload.json.gz");
    std::fs::write(&file, [0x1f, 0x8b, 0x08, 0x00]).unwrap();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/backups/upload")
            .header("Content-Type", "application/gzip");
        then.status(201)
            .json_body(json!({"backup": {"backup_id": "b2", "status": "completed"}}));
    });

    dk().args(["--url", &server.base_url(), "admin", "backup-upload"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("b2"));
    m.assert();
    let _ = std::fs::remove_file(&file);
}

#[test]
fn backup_restore_targets_namespaces() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/backups/restore")
            .json_body(json!({"backup_id": "b1", "target_namespaces": ["a"]}));
        then.status(202).json_body(json!({
            "restore_id": "r1", "status": "inprogress", "backup_id": "b1",
            "namespaces": ["a"], "started_at": 1
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-restore",
        "b1",
        "-n",
        "a",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Restore 'r1' started"))
    .stdout(predicate::str::contains("does not pause serving"));
    m.assert();
}

#[test]
fn backup_restore_overwrite_needs_yes_and_sends_no_request() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "admin",
        "backup-restore",
        "b1",
        "--overwrite",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("add --yes"));
}

#[test]
fn backup_restore_overwrite_with_yes_and_wait() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/backups/restore")
            .json_body(json!({"backup_id": "b1", "overwrite": true}));
        then.status(202).json_body(json!({
            "restore_id": "r1", "status": "inprogress", "backup_id": "b1",
            "namespaces": [], "started_at": 1
        }));
    });
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/restore/r1");
        then.status(200).json_body(json!({
            "restore_id": "r1", "status": "completed", "backup_id": "b1",
            "namespaces": [], "started_at": 1, "progress_percent": 100
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-restore",
        "b1",
        "--overwrite",
        "--yes",
        "--wait",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("completed"));
    m.assert();
}

#[test]
fn backup_restore_status_reads_the_restore() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/restore/r1");
        then.status(200).json_body(json!({
            "restore_id": "r1", "status": "inprogress", "backup_id": "b1",
            "namespaces": [], "started_at": 1, "progress_percent": 40
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-restore-status",
        "r1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("inprogress"));
}

#[test]
fn backup_schedule_get_and_set() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/admin/backups/schedule");
        then.status(200).json_body(json!({
            "enabled": true, "cron": "0 3 * * *", "backup_type": "full",
            "retention_days": 7, "max_backups": 5, "namespaces": [], "encrypt": false
        }));
    });
    let set = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/backups/schedule")
            .json_body(json!({"enabled": false}));
        then.status(200).json_body(json!({
            "enabled": false, "backup_type": "full", "retention_days": 7, "max_backups": 5,
            "namespaces": [], "encrypt": false
        }));
    });

    dk().args(["--url", &server.base_url(), "admin", "backup-schedule"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 3 * * *"));
    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "backup-schedule",
        "--set",
        "{\"enabled\": false}",
    ])
    .assert()
    .success();
    set.assert();
}

#[test]
fn backup_schedule_set_rejects_invalid_json() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "admin",
        "backup-schedule",
        "--set",
        "{nope",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("not valid JSON"));
}

#[test]
fn quotas_set_without_a_namespace_targets_the_default_quota() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PUT)
            .path("/admin/quotas/default")
            .json_body(json!({"config": {"max_vectors": 1000, "enforcement": "hard"}}));
        then.status(200).json_body(json!({
            "success": true, "namespace": "_default",
            "config": {"max_vectors": 1000, "enforcement": "hard"}, "message": "ok"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "quotas-set",
        "--data",
        "{\"max_vectors\": 1000, \"enforcement\": \"hard\"}",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Quotas updated"));
    m.assert();
}

#[test]
fn a_quota_413_exits_5_and_points_at_quotas_get() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(PUT).path("/admin/quotas/docs");
        then.status(413).json_body(json!({
            "error": "quota exceeded", "code": "QUOTA_EXCEEDED", "status": 413
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "admin",
        "quotas-set",
        "-n",
        "docs",
        "--data",
        "{\"max_vectors\": 1}",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("quotas-get"));
}

// ---------------------------------------------------------------------------
// Routes the v0.12 server serves (sweep of every route `dk` calls)
// ---------------------------------------------------------------------------

#[test]
fn memory_update_calls_the_update_route_with_the_agent_in_the_query() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PUT)
            .path("/v1/memory/update/mem-1")
            .query_param("agent_id", "bot")
            .json_body(json!({"content": "new text", "memory_type": "semantic"}));
        then.status(200).json_body(json!({"id": "mem-1"}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "update",
        "bot",
        "mem-1",
        "--content",
        "new text",
        "--type",
        "semantic",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory 'mem-1' updated"));
    m.assert();
}

#[test]
fn memory_feedback_sends_a_signal() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/feedback")
            .json_body(json!({"agent_id": "bot", "memory_id": "mem-1", "signal": "downvote"}));
        then.status(200).json_body(json!({
            "memory_id": "mem-1", "new_importance": 0.425, "signal": "downvote"
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "feedback",
        "bot",
        "mem-1",
        "downvote",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("signal: downvote"))
    .stdout(predicate::str::contains("0.425"));
    m.assert();
}

#[test]
fn memory_feedback_rejects_free_text() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "memory",
        "feedback",
        "bot",
        "mem-1",
        "very relevant",
    ])
    .assert()
    .failure();
}

#[test]
fn batch_forget_deletes_with_a_filter_object() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/memories/forget/batch")
            .json_body(json!({
                "agent_id": "bot",
                "filter": {"memory_type": "working", "min_importance": 0.5}
            }));
        then.status(200).json_body(json!({"deleted_count": 4}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-forget",
        "bot",
        "--type",
        "working",
        "--min-importance",
        "0.5",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Deleted 4"));
    m.assert();
}

#[test]
fn batch_forget_max_age_becomes_created_before() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/memories/forget/batch")
            .json_body_includes(json!({"agent_id": "bot"}).to_string());
        then.status(200).json_body(json!({"deleted_count": 0}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-forget",
        "bot",
        "--max-age-days",
        "30",
    ])
    .assert()
    .success();
    let hits = m.calls();
    assert_eq!(hits, 1);
}

#[test]
fn batch_forget_dry_run_counts_through_batch_recall() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memories/recall/batch")
            .json_body(json!({
                "agent_id": "bot",
                "filter": {"min_importance": 0.5},
                "limit": 1
            }));
        then.status(200).json_body(json!({
            "memories": [], "total": 9, "filtered": 7, "truncated": true
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-forget",
        "bot",
        "--min-importance",
        "0.5",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[dry-run] Would delete 7"));
    m.assert();
}

#[test]
fn text_search_uses_the_namespace_route_and_top_k() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/namespaces/docs/fulltext/search")
            .json_body(json!({"query": "needle", "top_k": 20}));
        then.status(200).json_body(json!({
            "results": [{"id": "d1", "score": 1.5, "metadata": {"content": "a needle"}}],
            "search_time_ms": 2
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "text",
        "search",
        "needle",
        "--namespace",
        "docs",
        "--limit",
        "20",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("1 result"))
    .stdout(predicate::str::contains("a needle"));
    m.assert();
}

#[test]
fn text_search_without_a_namespace_exits_5() {
    dk().args(["--url", "http://127.0.0.1:1", "text", "search", "needle"])
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("--namespace"));
}

#[test]
fn configure_ttl_is_gone() {
    dk().args(["admin", "configure-ttl", "docs", "--ttl-seconds", "60"])
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// memory: --lang, --attachment-ref, batch-store, extract (dakera-client 0.12)
// ---------------------------------------------------------------------------

#[test]
fn memory_store_sends_lang_and_attachment_ref() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store").json_body(json!({
            "agent_id": "bot",
            "content": "Anna kommt morgen",
            "memory_type": "episodic",
            "importance": 0.5,
            "tags": [],
            "lang": "de",
            "attachment_ref": "sha256:abc123"
        }));
        then.status(200)
            .json_body(json!({"memory_id": "mem-9", "namespace": "_dakera_agent_bot"}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "bot",
        "Anna kommt morgen",
        "--lang",
        "de",
        "--attachment-ref",
        "sha256:abc123",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("mem-9"));
    m.assert();
}

#[test]
fn memory_store_without_lang_sends_neither_field() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store").json_body(json!({
            "agent_id": "bot",
            "content": "plain",
            "memory_type": "episodic",
            "importance": 0.5,
            "tags": []
        }));
        then.status(200)
            .json_body(json!({"memory_id": "mem-1", "namespace": "_dakera_agent_bot"}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "bot",
        "plain",
    ])
    .assert()
    .success();
    m.assert();
}

#[test]
fn memory_recall_sends_lang() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/recall")
            .json_body(json!({
                "agent_id": "bot",
                "query": "quand",
                "top_k": 3,
                "min_importance": 0.0,
                "tags": [],
                "lang": "fr"
            }));
        then.status(200)
            .json_body(json!({"memories": [], "total_found": 0}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "recall",
        "bot",
        "quand",
        "--top-k",
        "3",
        "--lang",
        "fr",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No memories found"));
    m.assert();
}

#[test]
fn memory_search_sends_lang() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memory/search")
            .json_body(json!({
                "agent_id": "bot",
                "query": "cuando",
                "top_k": 10,
                "memory_type": "semantic",
                "min_importance": 0.0,
                "tags": [],
                "lang": "es"
            }));
        then.status(200)
            .json_body(json!({"memories": [], "total_found": 0}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "search",
        "bot",
        "cuando",
        "--type",
        "semantic",
        "--lang",
        "es",
    ])
    .assert()
    .success();
    m.assert();
}

#[test]
fn memory_update_sends_lang() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PUT)
            .path("/v1/memory/update/mem-1")
            .query_param("agent_id", "bot")
            .json_body(json!({"content": "Olá", "lang": "pt-BR"}));
        then.status(200).json_body(json!({"id": "mem-1"}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "update",
        "bot",
        "mem-1",
        "--content",
        "Olá",
        "--lang",
        "pt-BR",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Memory 'mem-1' updated"));
    m.assert();
}

#[test]
fn memory_batch_store_sends_contents_with_defaults_and_lang() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memories/store/batch").json_body(json!({
            "agent_id": "bot",
            "memories": [
                {"content": "Prefers dark mode", "memory_type": "episodic", "importance": 0.5, "tags": ["prefs"], "session_id": "s-1"},
                {"content": "Lives in Berlin", "memory_type": "episodic", "importance": 0.5, "tags": ["prefs"], "session_id": "s-1"}
            ],
            "lang": "en"
        }));
        then.status(200).json_body(json!({
            "stored": [
                {"id": "m-1", "content": "Prefers dark mode", "agent_id": "bot", "tags": ["prefs"], "importance": 0.5, "created_at": 1},
                {"id": "m-2", "content": "Lives in Berlin", "agent_id": "bot", "tags": ["prefs"], "importance": 0.5, "created_at": 1}
            ],
            "stored_count": 2,
            "total_embedding_time_ms": 7
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-store",
        "bot",
        "-c",
        "Prefers dark mode",
        "-c",
        "Lives in Berlin",
        "--tag",
        "prefs",
        "--session-id",
        "s-1",
        "--lang",
        "en",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Stored 2 memories"))
    .stdout(predicate::str::contains("m-2"));
    m.assert();
}

#[test]
fn memory_batch_store_reads_a_json_file_of_strings_and_objects() {
    let path = scratch("batch.json");
    std::fs::write(
        &path,
        json!([
            "first",
            {"content": "second", "importance": 0.9, "memory_type": "procedural",
             "tags": ["own"], "attachment_ref": "sha256:ff", "id": "custom-2"}
        ])
        .to_string(),
    )
    .unwrap();
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memories/store/batch").json_body(json!({
            "agent_id": "bot",
            "memories": [
                {"content": "first", "memory_type": "semantic", "importance": 0.25, "tags": []},
                {"content": "second", "memory_type": "procedural", "importance": 0.9, "tags": ["own"],
                 "attachment_ref": "sha256:ff", "id": "custom-2"}
            ]
        }));
        then.status(200).json_body(json!({
            "stored": [], "stored_count": 2, "total_embedding_time_ms": 3
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-store",
        "bot",
        "--file",
        path.to_str().unwrap(),
        "--type",
        "semantic",
        "--importance",
        "0.25",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Stored 2 memories"));
    m.assert();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn memory_batch_store_reads_stdin() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/v1/memories/store/batch").json_body(json!({
            "agent_id": "bot",
            "memories": [
                {"content": "from stdin", "memory_type": "episodic", "importance": 0.5, "tags": []}
            ]
        }));
        then.status(200).json_body(json!({
            "stored": [], "stored_count": 1, "total_embedding_time_ms": 1
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "batch-store",
        "bot",
        "--file",
        "-",
    ])
    .write_stdin(r#"["from stdin"]"#)
    .assert()
    .success();
    m.assert();
}

#[test]
fn memory_batch_store_rejects_bad_input_before_calling_the_server() {
    // Nothing to store.
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "memory",
        "batch-store",
        "bot",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("nothing to store"));

    // Not an array.
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "memory",
        "batch-store",
        "bot",
        "--file",
        "-",
    ])
    .write_stdin(r#"{"content": "x"}"#)
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("JSON array"));

    // An object without content.
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "memory",
        "batch-store",
        "bot",
        "--file",
        "-",
    ])
    .write_stdin(r#"[{"importance": 0.3}]"#)
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("item 0"));
}

#[test]
fn memory_extract_sends_types_and_lang_and_prints_entities() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memories/extract")
            .json_body(json!({
                "content": "Anna kommt morgen nach Berlin",
                "entity_types": ["person", "location"],
                "lang": "de"
            }));
        then.status(200).json_body(json!({
            "entities": [
                {"entity_type": "person", "value": "Anna", "score": 0.93},
                {"entity_type": "location", "value": "Berlin", "score": 0.88}
            ],
            "count": 2
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "extract",
        "Anna kommt morgen nach Berlin",
        "--entity-types",
        "person,location",
        "--lang",
        "de",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Found 2 entities"))
    .stdout(predicate::str::contains("Berlin"));
    m.assert();
}

#[test]
fn memory_extract_without_types_omits_entity_types() {
    // The v0.12.0 server answers `"entity_types": null` with a 422.
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/memories/extract")
            .json_body(json!({"content": "nothing here"}));
        then.status(200)
            .json_body(json!({"entities": [], "count": 0}));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "extract",
        "nothing here",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No entities found"));
    m.assert();
}

// Exit codes of the SDK-backed commands follow the HTTP status, like the
// raw-REST ones (a 400 used to exit 6 and a 501 exit 1).

#[test]
fn sdk_command_400_exits_5() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(400).json_body(json!({
            "error": "Invalid request: unsupported lang 'xx': supported languages are en, de, fr, es, it, pt, nl",
            "code": "INVALID_REQUEST",
            "status": 400
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "bot",
        "x",
        "--lang",
        "xx",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("unsupported lang"));
}

#[test]
fn sdk_command_501_feature_disabled_exits_6() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/memory/store");
        then.status(501).json_body(json!({
            "error": "The attachments API is not enabled on this server (set DAKERA_ATTACHMENTS to enable it)",
            "code": "FEATURE_DISABLED",
            "status": 501
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "memory",
        "store",
        "bot",
        "x",
        "--attachment-ref",
        "sha256:00",
    ])
    .assert()
    .failure()
    .code(6)
    .stderr(predicate::str::contains("DAKERA_ATTACHMENTS"));
}

#[test]
fn sdk_command_404_exits_3() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/memory/get/nope");
        then.status(404).json_body(json!({
            "error": "Memory not found: nope",
            "code": "VECTOR_NOT_FOUND",
            "status": 404
        }));
    });

    dk().args(["--url", &server.base_url(), "memory", "get", "bot", "nope"])
        .assert()
        .failure()
        .code(3);
}

// ---------------------------------------------------------------------------
// knowledge: the v0.12.0 server's request and answer shapes (raw REST)
// ---------------------------------------------------------------------------

#[test]
fn knowledge_graph_sends_the_seed_and_prints_related() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/knowledge/graph")
            .json_body(json!({
                "agent_id": "bot", "memory_id": "mem-1", "depth": 2
            }));
        then.status(200).json_body(json!({
            "root": {
                "memory": {"id": "mem-1", "content": "Anna leads Alpha", "memory_type": "episodic",
                           "agent_id": "bot", "importance": 0.5, "tags": [], "created_at": 1},
                "similarity": 1.0,
                "related": [{"memory_id": "mem-2", "similarity": 0.91, "shared_tags": ["alpha"]}]
            },
            "total_nodes": 2
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "graph",
        "bot",
        "--memory-id",
        "mem-1",
        "--depth",
        "2",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Knowledge graph from mem-1: 2 nodes",
    ))
    .stdout(predicate::str::contains("mem-2"))
    .stdout(predicate::str::contains("alpha"));
    m.assert();
}

#[test]
fn knowledge_full_graph_reads_the_server_answer() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/knowledge/graph/full")
            .json_body(json!({"agent_id": "bot", "max_nodes": 50, "max_edges_per_node": 3}));
        then.status(200).json_body(json!({
            "nodes": [
                {"id": "mem-1", "content": "a", "memory_type": "Episodic", "importance": 0.5,
                 "tags": [], "created_at": "1", "cluster_id": 0, "centrality": 1.0},
                {"id": "mem-2", "content": "b", "memory_type": "Episodic", "importance": 0.5,
                 "tags": [], "created_at": "1", "cluster_id": 0, "centrality": 1.0}
            ],
            "edges": [{"source": "mem-1", "target": "mem-2", "similarity": 0.9, "shared_tags": []}],
            "clusters": [{"id": 0, "node_count": 2, "top_tags": ["x"], "avg_importance": 0.5}],
            "stats": {"total_memories": 2, "included_memories": 2, "total_edges": 1,
                      "cluster_count": 1, "density": 1.0, "hub_memory_id": "mem-2"}
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "full-graph",
        "bot",
        "--max-nodes",
        "50",
        "--max-edges",
        "3",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("2 nodes, 1 edges"))
    .stdout(predicate::str::contains("Cluster 0: 2 nodes"))
    .stdout(predicate::str::contains("Hub memory: mem-2"));
    m.assert();
}

#[test]
fn knowledge_summarize_sends_ids_and_prints_the_new_memory() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/knowledge/summarize")
            .json_body(json!({
                "agent_id": "bot", "memory_ids": ["m1", "m2"], "target_type": "semantic"
            }));
        then.status(200).json_body(json!({
            "summary_memory": {"id": "mem-sum", "content": "Bob fixed and shipped the login bug",
                               "memory_type": "semantic", "agent_id": "bot", "importance": 0.6,
                               "tags": [], "created_at": 1},
            "source_count": 2
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "summarize",
        "bot",
        "--memory-ids",
        "m1, m2",
        "--target-type",
        "semantic",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Summarized 2 memories into mem-sum",
    ))
    .stdout(predicate::str::contains("shipped the login bug"));
    m.assert();
}

#[test]
fn knowledge_summarize_with_one_id_fails_before_the_request() {
    dk().args([
        "--url",
        "http://127.0.0.1:1",
        "knowledge",
        "summarize",
        "bot",
        "--memory-ids",
        "m1",
    ])
    .assert()
    .failure()
    .code(5)
    .stderr(predicate::str::contains("at least two"));
}

#[test]
fn knowledge_deduplicate_dry_run_reads_groups() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/knowledge/deduplicate")
            .json_body(json!({"agent_id": "bot", "dry_run": true, "threshold": 0.75}));
        then.status(200).json_body(json!({
            "groups": [{"canonical_id": "m1", "duplicate_ids": ["m2", "m3"], "avg_similarity": 0.97}],
            "duplicates_found": 2,
            "duplicates_merged": 0
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "knowledge",
        "deduplicate",
        "bot",
        "--threshold",
        "0.75",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "[dry-run] Found 2 duplicates in 1 groups",
    ))
    .stdout(predicate::str::contains("m2, m3"));
    m.assert();
}

#[test]
fn index_fulltext_stats_prints_the_server_answer() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/ns-1/fulltext/stats");
        then.status(200).json_body(json!({
            "document_count": 2, "unique_terms": 5, "avg_doc_length": 4.5
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "--format",
        "json",
        "index",
        "fulltext-stats",
        "--namespace",
        "ns-1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"unique_terms\": 5"));
    m.assert();
}

// ---------------------------------------------------------------------------
// namespace create / delete and index rebuild call the server (they were stubs)
// ---------------------------------------------------------------------------

#[test]
fn namespace_create_puts_dimension_and_distance() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PUT)
            .path("/v1/namespaces/docs")
            .json_body(json!({"dimension": 384, "distance": "dot"}));
        then.status(200).json_body(json!({
            "namespace": "docs", "dimension": 384, "distance": "dot", "created": true
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "create",
        "docs",
        "--dimension",
        "384",
        "--distance",
        "dot",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Namespace 'docs' created (dimension 384, distance dot)",
    ));
    m.assert();
}

#[test]
fn namespace_create_reports_an_existing_namespace() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(PUT).path("/v1/namespaces/docs");
        then.status(200).json_body(json!({
            "namespace": "docs", "dimension": 384, "distance": "cosine", "created": false
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "create",
        "docs",
        "--dimension",
        "384",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("already exists"));
}

#[test]
fn namespace_create_needs_a_dimension() {
    dk().args(["--url", "http://127.0.0.1:1", "namespace", "create", "docs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--dimension"));
}

#[test]
fn namespace_delete_calls_delete() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(DELETE).path("/v1/namespaces/docs");
        then.status(200).json_body(json!({
            "success": true, "namespace": "docs", "vectors_deleted": 12
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "namespace",
        "delete",
        "docs",
        "--yes",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Namespace 'docs' deleted (12 vectors)",
    ));
    m.assert();
}

#[test]
fn index_rebuild_posts_to_the_admin_route() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/admin/indexes/rebuild")
            .json_body(json!({"namespace": "docs", "force": true}));
        then.status(200).json_body(json!({
            "success": true,
            "job_id": "job_1",
            "message": "1 namespace(s) on this node: 1 ANN index(es) rebuilt",
            "namespaces": []
        }));
    });

    dk().args([
        "--url",
        &server.base_url(),
        "index",
        "rebuild",
        "-n",
        "docs",
        "--force",
        "--yes",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("1 ANN index(es) rebuilt"));
    m.assert();
}
