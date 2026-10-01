//! Operator commands added for Dakera v0.12, all under `dk admin`:
//! the embed migration, encryption status and keyring rotation, and the
//! backup routes (create, get, download, upload, restore, schedule).
//!
//! Permissions (v0.12): every route here needs a *global* key. A key pinned to
//! namespaces gets `403` even with the admin scope, and backup download,
//! upload and restore need `super_admin`. The server's answer is turned into
//! a message that says so (see `crate::api::ApiHttpError`).

use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde_json::{json, Value};

use super::capabilities::show;
use super::health::migration_line;
use crate::api::{self, ptr_str, str_of, ApiHttpError};
use crate::context::Context;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

fn is_table(ctx: &Context) -> bool {
    matches!(ctx.format, OutputFormat::Table)
}

async fn get(ctx: &Context, path: &str) -> Result<Value> {
    api::request_json_logged(ctx, Method::GET, path, None).await
}

async fn post(ctx: &Context, path: &str, body: &Value) -> Result<Value> {
    api::request_json_logged(ctx, Method::POST, path, Some(body)).await
}

// ---------------------------------------------------------------------------
// Embed migration
// ---------------------------------------------------------------------------

/// `dk admin embed-migration` — `GET /admin/reembed/migration`.
///
/// After an upgrade the stored memories are re-embedded once in the
/// background (they were embedded with the query instruction); recall is
/// served throughout. `/health` shows the same summary under `embed_migration`.
pub async fn embed_migration(ctx: &Context) -> Result<()> {
    let status = get(ctx, "/admin/reembed/migration").await?;
    if !is_table(ctx) {
        output::print_item(&status, ctx.format);
        return Ok(());
    }
    output::info(&migration_line(&status));
    if let Some(reason) = str_of(&status, "reason") {
        println!("  Reason: {reason}");
    }
    let rate = show(&status, "/rate_per_sec");
    let target_model = show(&status, "/target/model");
    let target_side = show(&status, "/target/side");
    let pairs = vec![
        ("State", show(&status, "/state")),
        ("Remaining", show(&status, "/remaining")),
        ("Re-embedded", show(&status, "/reembedded")),
        ("Skipped", show(&status, "/skipped")),
        ("Rate (per second)", rate),
        ("ETA (seconds)", show(&status, "/eta_secs")),
        ("Target model", target_model),
        ("Target side", target_side),
    ];
    output::print_kv(&pairs, ctx.format);
    if let Some(namespaces) = status.get("namespaces").and_then(|n| n.as_object()) {
        let rows: Vec<Value> = namespaces
            .iter()
            .map(|(name, progress)| namespace_progress_row(name, progress))
            .collect();
        if !rows.is_empty() {
            output::print_data(&rows, ctx.format);
        }
    }
    Ok(())
}

fn namespace_progress_row(name: &str, progress: &Value) -> Value {
    json!({
        "namespace": name,
        "remaining": show(progress, "/remaining"),
        "reembedded": show(progress, "/reembedded"),
        "skipped": show(progress, "/skipped"),
        "done": show(progress, "/done"),
    })
}

// ---------------------------------------------------------------------------
// Encryption: status, keyring rotation, re-seal
// ---------------------------------------------------------------------------

/// `dk admin encryption-status` — `GET /admin/encryption/status`.
pub async fn encryption_status(ctx: &Context) -> Result<()> {
    let status = get(ctx, "/admin/encryption/status").await?;
    if !is_table(ctx) {
        output::print_item(&status, ctx.format);
        return Ok(());
    }
    let enabled = show(&status, "/enabled");
    if enabled != "true" {
        output::info("Encryption at rest is not configured (DAKERA_ENCRYPTION_KEY is not set)");
        return Ok(());
    }
    let pairs = vec![
        ("Enabled", enabled),
        ("Node", show(&status, "/node_id")),
        ("Default key", show(&status, "/default_key_id")),
        ("Environment key", show(&status, "/environment_key_id")),
        ("Re-seal passes", show(&status, "/reseal/passes_completed")),
        (
            "Re-seal retry pending",
            show(&status, "/reseal/retry_pending"),
        ),
    ];
    output::print_kv(&pairs, ctx.format);
    if let Some(current) = status.pointer("/reseal/current") {
        if !current.is_null() {
            println!("  Re-seal in progress: {current}");
        }
    }
    if let Some(keys) = status.get("keys").and_then(|k| k.as_array()) {
        let rows: Vec<Value> = keys.iter().map(key_row).collect();
        output::print_data(&rows, ctx.format);
    }
    if let Some(map) = status.get("namespace_keys").and_then(|m| m.as_object()) {
        let rows: Vec<Value> = map
            .iter()
            .map(|(ns, key)| json!({ "namespace": ns, "key_id": key }))
            .collect();
        if !rows.is_empty() {
            println!();
            output::info("Namespaces with their own key");
            output::print_data(&rows, ctx.format);
        }
    }
    Ok(())
}

fn key_row(key: &Value) -> Value {
    json!({
        "key_id": show(key, "/key_id"),
        "origin": show(key, "/origin"),
        "active_for": show(key, "/active_for"),
        "retired_at_ms": show(key, "/retired_at_ms"),
        "values_on_this_node": show(key, "/referenced_on_this_node"),
    })
}

/// The body of `POST /admin/encryption/rotate-key`.
///
/// The new key is generated by the server unless `--new-key-env` names an
/// environment variable that holds a passphrase or a 64-character hex key (a
/// key on the command line would end up in shell history).
pub fn rotate_body(sub: &ArgMatches) -> Result<Value> {
    let mut body = json!({});
    if let Some(ns) = sub.get_one::<String>("namespace") {
        body["namespace"] = json!(ns);
    }
    if let Some(var) = sub.get_one::<String>("new-key-env") {
        match std::env::var(var) {
            Ok(key) if !key.is_empty() => body["new_key"] = json!(key),
            _ => {
                return Err(input_error(format!(
                    "environment variable {var} is not set"
                )))
            }
        }
    }
    if let Some(secs) = sub.get_one::<u64>("wait-secs") {
        body["wait_secs"] = json!(secs);
    }
    Ok(body)
}

/// `dk admin encryption-rotate` — `POST /admin/encryption/rotate-key`.
pub async fn encryption_rotate(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let body = rotate_body(sub)?;
    let result = post(ctx, "/admin/encryption/rotate-key", &body).await?;
    if !is_table(ctx) {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    match str_of(&result, "namespace") {
        Some(ns) => output::success(&format!("Rotated the encryption key of namespace '{ns}'")),
        None => output::success("Rotated the global encryption key"),
    }
    let pairs = vec![
        ("Key id", show(&result, "/key_id")),
        ("Previous key id", show(&result, "/previous_key_id")),
        ("Scope", show(&result, "/scope")),
        ("Re-seal", show(&result, "/reseal")),
        ("Values re-sealed", show(&result, "/rotated")),
        ("Values skipped", show(&result, "/skipped")),
    ];
    output::print_kv(&pairs, ctx.format);
    if let Some(failed) = result.get("failed_namespaces").and_then(|f| f.as_array()) {
        if !failed.is_empty() {
            let names = show(&result, "/failed_namespaces");
            output::warning(&format!("Re-seal failed for: {names}"));
        }
    }
    if let Some(action) = str_of(&result, "action_required") {
        output::warning(action);
    }
    if str_of(&result, "reseal") == Some("running") {
        output::info("The re-seal continues in the background: `dk admin encryption-status`");
    }
    Ok(())
}

/// `dk admin encryption-reseal` — `POST /admin/encryption/reseal`.
pub async fn encryption_reseal(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let mut body = json!({});
    if let Some(ns) = sub.get_one::<String>("namespace") {
        body["namespace"] = json!(ns);
    }
    if let Some(secs) = sub.get_one::<u64>("wait-secs") {
        body["wait_secs"] = json!(secs);
    }
    let result = post(ctx, "/admin/encryption/reseal", &body).await?;
    if is_table(ctx) {
        if str_of(&result, "reseal") == Some("running") {
            output::info("Re-seal pass started; it continues in the background");
        } else {
            output::success("Re-seal pass finished");
        }
    }
    output::print_item(&result, ctx.format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Backups
// ---------------------------------------------------------------------------

fn backup_path(backup_id: &str) -> String {
    format!("/admin/backups/{}", api::segment(backup_id))
}

fn default_backup_name() -> String {
    let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };
    format!("dk-backup-{secs}")
}

/// The body of `POST /admin/backups`.
pub fn backup_create_body(sub: &ArgMatches) -> Value {
    let name = match sub.get_one::<String>("name") {
        Some(n) => n.clone(),
        None => default_backup_name(),
    };
    let mut body = json!({ "name": name });
    if let Some(kind) = sub.get_one::<String>("type") {
        body["backup_type"] = json!(kind);
    }
    if let Some(namespaces) = sub.get_many::<String>("namespace") {
        let namespaces: Vec<&String> = namespaces.collect();
        body["namespaces"] = json!(namespaces);
    }
    if sub.get_flag("encrypt") {
        body["encrypt"] = json!(true);
    }
    if let Some(compression) = sub.get_one::<String>("compression") {
        body["compression"] = json!(compression);
    }
    body
}

/// `dk admin backup-create` — `POST /admin/backups` (answers `202`; the
/// backup runs in the background).
pub async fn backup_create(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let body = backup_create_body(sub);
    let result = post(ctx, "/admin/backups", &body).await?;
    let backup_id = ptr_str(&result, "/backup/backup_id").unwrap_or("");
    let backup_id = backup_id.to_string();
    if is_table(ctx) {
        output::success(&format!("Backup '{backup_id}' started"));
    }
    if !sub.get_flag("wait") || backup_id.is_empty() {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    let timeout = sub.get_one::<u64>("timeout").copied().unwrap_or(3600);
    let finished = wait_for_backup(ctx, &backup_id, timeout).await?;
    output::print_item(&finished, ctx.format);
    Ok(())
}

/// Poll `GET /admin/backups/{id}` until the backup completes or fails.
async fn wait_for_backup(ctx: &Context, backup_id: &str, timeout_secs: u64) -> Result<Value> {
    let path = backup_path(backup_id);
    let started = Instant::now();
    loop {
        let backup = get(ctx, &path).await?;
        match str_of(&backup, "status") {
            Some("completed") => return Ok(backup),
            Some("failed") => return Err(failed(&backup, "the backup failed")),
            _ => {}
        }
        if started.elapsed().as_secs() >= timeout_secs {
            anyhow::bail!("Timed out after {timeout_secs}s waiting for backup '{backup_id}'");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// A failed backup or restore as an error: the entry's `error` text.
fn failed(entry: &Value, fallback: &str) -> anyhow::Error {
    let message = match str_of(entry, "error") {
        Some(e) => e.to_string(),
        None => fallback.to_string(),
    };
    let err = ApiHttpError {
        status: 500,
        status_text: "background job failed".to_string(),
        code: None,
        message,
        details: None,
        retry_after_secs: None,
    };
    anyhow::Error::new(err)
}

/// `dk admin backup-get` — `GET /admin/backups/{id}`.
pub async fn backup_get(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let backup_id = sub.get_one::<String>("backup_id").unwrap();
    let backup = get(ctx, &backup_path(backup_id)).await?;
    output::print_item(&backup, ctx.format);
    Ok(())
}

/// `dk admin backup-download` — `GET /admin/backups/{id}/download`
/// (`super_admin`; a gzip-compressed JSON bundle).
pub async fn backup_download(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let backup_id = sub.get_one::<String>("backup_id").unwrap();
    let output_path = sub.get_one::<String>("output").unwrap();
    let path = format!("{}/download", backup_path(backup_id));
    let bytes = api::get_bytes(ctx, &path).await?;
    if output_path == "-" {
        let mut stdout = std::io::stdout();
        stdout.write_all(&bytes)?;
        stdout.flush()?;
        return Ok(());
    }
    if let Err(e) = std::fs::write(output_path, &bytes) {
        return Err(input_error(format!("failed to write {output_path}: {e}")));
    }
    if is_table(ctx) {
        output::success(&format!("Saved {} bytes to {output_path}", bytes.len()));
    } else {
        let saved = json!({ "path": output_path, "size_bytes": bytes.len() });
        output::print_item(&saved, ctx.format);
    }
    Ok(())
}

/// The media type to upload a backup bundle as: gzip by its magic bytes.
pub fn bundle_content_type(data: &[u8]) -> &'static str {
    if data.starts_with(&GZIP_MAGIC) {
        "application/gzip"
    } else {
        "application/json"
    }
}

/// `dk admin backup-upload` — `POST /admin/backups/upload` (`super_admin`):
/// a bundle from `backup-download`, restored into the cluster at once.
pub async fn backup_upload(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let file = sub.get_one::<String>("file").unwrap();
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => return Err(input_error(format!("failed to read file {file}: {e}"))),
    };
    let content_type = bundle_content_type(&data);
    let result = api::post_bytes(ctx, "/admin/backups/upload", content_type, data).await?;
    if is_table(ctx) {
        output::success(&format!("Uploaded and restored {file}"));
    }
    output::print_item(&result, ctx.format);
    Ok(())
}

/// The body of `POST /admin/backups/restore`.
///
/// `--overwrite` restores the backup's point in time: what was written since
/// is removed. It needs `--yes`.
pub fn restore_body(sub: &ArgMatches) -> Result<Value> {
    let backup_id = sub.get_one::<String>("backup_id").unwrap();
    let mut body = json!({ "backup_id": backup_id });
    if let Some(namespaces) = sub.get_many::<String>("namespace") {
        let namespaces: Vec<&String> = namespaces.collect();
        body["target_namespaces"] = json!(namespaces);
    }
    if sub.get_flag("overwrite") {
        if !sub.get_flag("yes") {
            return Err(input_error(
                "--overwrite removes what was written since the backup; add --yes to confirm",
            ));
        }
        body["overwrite"] = json!(true);
    }
    Ok(body)
}

/// `dk admin backup-restore` — `POST /admin/backups/restore` (`super_admin`;
/// answers `202`, the restore runs in the background).
pub async fn backup_restore(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let body = restore_body(sub)?;
    let result = post(ctx, "/admin/backups/restore", &body).await?;
    let restore_id = str_of(&result, "restore_id").unwrap_or("").to_string();
    if is_table(ctx) {
        output::success(&format!("Restore '{restore_id}' started"));
        output::warning("Restore does not pause serving: run it in a maintenance window");
    }
    if !sub.get_flag("wait") || restore_id.is_empty() {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    let timeout = sub.get_one::<u64>("timeout").copied().unwrap_or(3600);
    let finished = wait_for_restore(ctx, &restore_id, timeout).await?;
    output::print_item(&finished, ctx.format);
    Ok(())
}

fn restore_path(restore_id: &str) -> String {
    format!("/admin/backups/restore/{}", api::segment(restore_id))
}

/// Poll `GET /admin/backups/restore/{id}` until the restore ends.
async fn wait_for_restore(ctx: &Context, restore_id: &str, timeout_secs: u64) -> Result<Value> {
    let path = restore_path(restore_id);
    let started = Instant::now();
    loop {
        let restore = get(ctx, &path).await?;
        match str_of(&restore, "status") {
            Some("completed") => return Ok(restore),
            Some("failed") => return Err(failed(&restore, "the restore failed")),
            _ => {}
        }
        if started.elapsed().as_secs() >= timeout_secs {
            anyhow::bail!("Timed out after {timeout_secs}s waiting for restore '{restore_id}'");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// `dk admin backup-restore-status` — `GET /admin/backups/restore/{id}`.
pub async fn backup_restore_status(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let restore_id = sub.get_one::<String>("restore_id").unwrap();
    let status = get(ctx, &restore_path(restore_id)).await?;
    output::print_item(&status, ctx.format);
    Ok(())
}

/// `dk admin backup-schedule` — `GET` or (with `--set`) `POST`
/// `/admin/backups/schedule`. From v0.12 an enabled stored schedule runs at
/// its next slot; v0.11 stored it and never ran it.
pub async fn backup_schedule(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let path = "/admin/backups/schedule";
    let result = match sub.get_one::<String>("set") {
        Some(data) => {
            let parsed: Result<Value, _> = serde_json::from_str(data);
            let body = match parsed {
                Ok(v) => v,
                Err(e) => return Err(input_error(format!("--set is not valid JSON: {e}"))),
            };
            post(ctx, path, &body).await?
        }
        None => get(ctx, path).await?,
    };
    output::print_item(&result, ctx.format);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::build_admin_command;

    fn matches_of(args: &str) -> ArgMatches {
        let m = build_admin_command()
            .try_get_matches_from(args.split_whitespace())
            .unwrap();
        let (_, sub) = m.subcommand().unwrap();
        sub.clone()
    }

    #[test]
    fn rotate_body_defaults_to_an_empty_object() {
        let sub = matches_of("admin encryption-rotate");
        assert_eq!(rotate_body(&sub).unwrap(), json!({}));
    }

    #[test]
    fn rotate_body_scopes_to_one_namespace() {
        let sub = matches_of("admin encryption-rotate -n team-a --wait-secs 30");
        let body = rotate_body(&sub).unwrap();
        assert_eq!(body["namespace"], "team-a");
        assert_eq!(body["wait_secs"], 30);
        assert!(body.get("new_key").is_none());
    }

    #[test]
    fn rotate_body_refuses_an_unset_key_variable() {
        let sub = matches_of("admin encryption-rotate --new-key-env DK_TEST_NO_SUCH");
        let err = rotate_body(&sub).unwrap_err();
        assert!(err.to_string().contains("DK_TEST_NO_SUCH"));
    }

    #[test]
    fn backup_create_body_has_the_required_name() {
        let sub = matches_of("admin backup-create");
        let body = backup_create_body(&sub);
        assert!(body["name"].as_str().unwrap().starts_with("dk-backup-"));
        assert!(body.get("encrypt").is_none());
    }

    #[test]
    fn backup_create_body_carries_the_options() {
        let line = "admin backup-create --name nightly --type snapshot -n a -n b";
        let sub = matches_of(&format!("{line} --encrypt --compression zstd"));
        let body = backup_create_body(&sub);
        assert_eq!(body["name"], "nightly");
        assert_eq!(body["backup_type"], "snapshot");
        assert_eq!(body["namespaces"], json!(["a", "b"]));
        assert_eq!(body["encrypt"], true);
        assert_eq!(body["compression"], "zstd");
    }

    #[test]
    fn restore_body_lists_target_namespaces() {
        let sub = matches_of("admin backup-restore bk1 -n a");
        let body = restore_body(&sub).unwrap();
        assert_eq!(body["backup_id"], "bk1");
        assert_eq!(body["target_namespaces"], json!(["a"]));
        assert!(body.get("overwrite").is_none());
    }

    #[test]
    fn restore_overwrite_needs_confirmation() {
        let sub = matches_of("admin backup-restore bk1 --overwrite");
        assert!(restore_body(&sub).is_err());
        let sub = matches_of("admin backup-restore bk1 --overwrite --yes");
        assert_eq!(restore_body(&sub).unwrap()["overwrite"], true);
    }

    #[test]
    fn bundle_content_type_follows_the_gzip_magic() {
        assert_eq!(bundle_content_type(&[0x1f, 0x8b, 0x08]), "application/gzip");
        assert_eq!(bundle_content_type(b"{\"backup\":{}}"), "application/json");
        assert_eq!(bundle_content_type(&[]), "application/json");
    }

    #[test]
    fn backup_paths_are_encoded() {
        assert_eq!(backup_path("abc-123"), "/admin/backups/abc-123");
        assert_eq!(restore_path("r 1"), "/admin/backups/restore/r%201");
    }
}
