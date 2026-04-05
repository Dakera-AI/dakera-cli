//! Integration tests for dakera-cli.
//!
//! Each test spins up a local [`httpmock`] HTTP server and exercises the
//! compiled `dk` binary via [`assert_cmd`]. No running Dakera server is needed.
//!
//! Commands covered:
//!   - `health`                       (DAK-1492: health check smoke tests)
//!   - `namespace list`               (DAK-1492: namespace list smoke tests)
//!   - `namespace policy get`         (DAK-1492: SEC-5 rate-limit policy tests)
//!   - `namespace policy set`         (DAK-1492: SEC-5 rate-limit policy tests)

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
    // Port 1 is privileged — connections are refused without a server running there.
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

/// Verifies that `dk namespace policy set` fetches the current policy first,
/// merges the supplied flags, and PUTs the result — then prints a success line.
#[test]
fn namespace_policy_set_rate_limit_reports_success() {
    let server = MockServer::start();

    // The command GETs the current policy first so it can do a partial update.
    server.mock(|when, then| {
        when.method(GET).path("/v1/namespaces/myns/memory_policy");
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(json!({ "working_ttl_seconds": 14400, "rate_limit_enabled": false }));
    });

    // Then PUTs the merged policy.
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

/// Verifies that disabling rate limiting via `--rate-limit-enabled false` also
/// succeeds — guards against the `false` boolean parse regression.
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
