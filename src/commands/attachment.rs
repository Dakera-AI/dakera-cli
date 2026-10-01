//! `dk attachment` — files attached to memories (Dakera v0.12, opt-in).
//!
//! The routes answer `501 FEATURE_DISABLED` until the server sets
//! `DAKERA_ATTACHMENTS` (speech to text) and, for image indexing,
//! `DAKERA_VISION`; `dk capabilities` shows what is on.
//!
//! * `upload`     `POST   /v1/namespaces/{ns}/attachments` (raw body + `Content-Type`)
//! * `list`       `GET    /v1/namespaces/{ns}/attachments`
//! * `download`   `GET    /v1/namespaces/{ns}/attachments/{ref}`
//! * `delete`     `DELETE /v1/namespaces/{ns}/attachments/{ref}`
//! * `transcribe` `POST   /v1/namespaces/{ns}/attachments/{ref}/transcribe`
//! * `index`      `POST   /v1/namespaces/{ns}/attachments/{ref}/index`
//! * `job`        `GET    …/transcribe/{job_id}` or `…/index/{job_id}`

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde_json::{json, Value};

use crate::api::{self, ptr_str, ptr_u64, str_of, u64_of, ApiHttpError};
use crate::context::Context;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("upload", sub)) => upload(ctx, sub).await,
        Some(("list", sub)) => list(ctx, sub).await,
        Some(("download", sub)) => download(ctx, sub).await,
        Some(("delete", sub)) => delete(ctx, sub).await,
        Some(("transcribe", sub)) => start_job(ctx, sub, "transcribe").await,
        Some(("index", sub)) => start_job(ctx, sub, "index").await,
        Some(("job", sub)) => job(ctx, sub).await,
        _ => {
            output::error("Unknown attachment subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }
}

/// The media type to upload a file as, from its extension.
pub fn guess_content_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn attachments_path(namespace: &str) -> String {
    format!("/v1/namespaces/{}/attachments", api::segment(namespace))
}

fn attachment_path(namespace: &str, reference: &str) -> String {
    let base = attachments_path(namespace);
    let reference = api::segment(reference);
    format!("{base}/{reference}")
}

async fn upload(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let file = sub.get_one::<String>("file").unwrap();
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => return Err(input_error(format!("failed to read file {file}: {e}"))),
    };
    let content_type = match sub.get_one::<String>("content-type") {
        Some(ct) => ct.as_str(),
        None => guess_content_type(file),
    };
    let path = attachments_path(namespace);
    let result = api::post_bytes(ctx, &path, content_type, data).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    match str_of(&result, "attachment_ref") {
        Some(reference) => output::success(&format!("Uploaded {file} as {reference}")),
        None => output::success(&format!("Uploaded {file}")),
    }
    output::print_item(&result, ctx.format);
    Ok(())
}

async fn list(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let path = attachments_path(namespace);
    let result = api::request_json_logged(ctx, Method::GET, &path, None).await?;
    let entries = result.get("attachments").and_then(|a| a.as_array());
    match entries {
        Some(items) if items.is_empty() && matches!(ctx.format, OutputFormat::Table) => {
            output::info(&format!("No attachments in namespace '{namespace}'"));
        }
        Some(items) => output::print_data(items, ctx.format),
        None => output::print_item(&result, ctx.format),
    }
    Ok(())
}

async fn download(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let reference = sub.get_one::<String>("reference").unwrap();
    let output_path = sub.get_one::<String>("output").unwrap();
    let path = attachment_path(namespace, reference);
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
    if matches!(ctx.format, OutputFormat::Table) {
        output::success(&format!("Saved {} bytes to {output_path}", bytes.len()));
    } else {
        let saved = json!({ "path": output_path, "size_bytes": bytes.len() });
        output::print_item(&saved, ctx.format);
    }
    Ok(())
}

async fn delete(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let reference = sub.get_one::<String>("reference").unwrap();
    let path = attachment_path(namespace, reference);
    api::request_json_logged(ctx, Method::DELETE, &path, None).await?;
    output::success(&format!("Attachment '{reference}' deleted from '{namespace}'"));
    Ok(())
}

/// The request body of a transcription or image-index job: the memory the
/// result becomes (`agent_id` is required, everything else has a default).
pub fn job_body(sub: &ArgMatches, kind: &str) -> Value {
    let agent_id = sub.get_one::<String>("agent-id").unwrap();
    let mut body = json!({ "agent_id": agent_id });
    if let Some(tags) = sub.get_many::<String>("tag") {
        let tags: Vec<&String> = tags.collect();
        body["tags"] = json!(tags);
    }
    if let Some(importance) = sub.get_one::<f64>("importance") {
        body["importance"] = json!(importance);
    }
    if let Some(ttl) = sub.get_one::<u64>("ttl-seconds") {
        body["ttl_seconds"] = json!(ttl);
    }
    let fields = [
        ("memory-type", "memory_type"),
        ("session-id", "session_id"),
        ("id", "id"),
        ("lang", "lang"),
    ];
    for (arg, field) in fields {
        if let Some(value) = sub.get_one::<String>(arg) {
            body[field] = json!(value);
        }
    }
    if kind == "index" {
        if let Some(content) = sub.get_one::<String>("content") {
            body["content"] = json!(content);
        }
    }
    body
}

async fn start_job(ctx: &Context, sub: &ArgMatches, kind: &str) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let reference = sub.get_one::<String>("reference").unwrap();
    let body = job_body(sub, kind);
    let path = format!("{}/{kind}", attachment_path(namespace, reference));
    let accepted = api::request_json_logged(ctx, Method::POST, &path, Some(&body)).await?;
    if !sub.get_flag("wait") {
        if matches!(ctx.format, OutputFormat::Table) {
            output::success(&format!("Job started ({kind}); poll it with `dk attachment job`"));
        }
        output::print_item(&accepted, ctx.format);
        return Ok(());
    }
    let status_path = match str_of(&accepted, "status_url") {
        Some(url) => url.to_string(),
        None => anyhow::bail!("the server's answer has no status_url to poll"),
    };
    let timeout = sub.get_one::<u64>("timeout").copied().unwrap_or(600);
    let finished = wait_for_job(ctx, &status_path, timeout).await?;
    output::print_item(&finished, ctx.format);
    Ok(())
}

async fn job(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let namespace = sub.get_one::<String>("namespace").unwrap();
    let reference = sub.get_one::<String>("reference").unwrap();
    let job_id = sub.get_one::<String>("job_id").unwrap();
    let kind = sub.get_one::<String>("kind").unwrap();
    let base = attachment_path(namespace, reference);
    let path = format!("{base}/{kind}/{}", api::segment(job_id));
    if sub.get_flag("wait") {
        let timeout = sub.get_one::<u64>("timeout").copied().unwrap_or(600);
        let finished = wait_for_job(ctx, &path, timeout).await?;
        output::print_item(&finished, ctx.format);
        return Ok(());
    }
    let status = api::request_json_logged(ctx, Method::GET, &path, None).await?;
    output::print_item(&status, ctx.format);
    Ok(())
}

/// Poll a job until it completes. A failed job becomes an error that carries
/// the HTTP status and code the synchronous request would have answered.
async fn wait_for_job(ctx: &Context, status_path: &str, timeout_secs: u64) -> Result<Value> {
    let started = Instant::now();
    let mut last = String::new();
    loop {
        let job = api::request_json_logged(ctx, Method::GET, status_path, None).await?;
        let status = str_of(&job, "status").unwrap_or("Unknown").to_string();
        let progress = u64_of(&job, "progress").unwrap_or(0);
        let message = str_of(&job, "message").unwrap_or("");
        let line = format!("{status} {progress}% {message}");
        if line != last {
            eprintln!("  {line}");
            last = line;
        }
        match status.as_str() {
            "Completed" => return Ok(job),
            "Failed" | "Cancelled" => return Err(job_failure(&job)),
            _ => {}
        }
        if started.elapsed().as_secs() >= timeout_secs {
            anyhow::bail!(
                "Timed out after {timeout_secs}s waiting for the job (last status: {status}); \
                 poll it with `dk attachment job`"
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// A failed job as an error: its `error` object holds the status and code.
fn job_failure(job: &Value) -> anyhow::Error {
    let status = ptr_u64(job, "/error/status").unwrap_or(500);
    let status = u16::try_from(status).unwrap_or(500);
    let code = ptr_str(job, "/error/code").map(|c| c.to_string());
    let message = match str_of(job, "message") {
        Some(m) => m.to_string(),
        None => "the job failed".to_string(),
    };
    let err = ApiHttpError {
        status,
        status_text: format!("job failed, status {status}"),
        code,
        message,
        details: None,
        retry_after_secs: None,
    };
    anyhow::Error::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::build_attachment_command;

    #[test]
    fn content_type_follows_the_extension() {
        assert_eq!(guess_content_type("note.wav"), "audio/wav");
        assert_eq!(guess_content_type("NOTE.WAV"), "audio/wav");
        assert_eq!(guess_content_type("page.png"), "image/png");
        assert_eq!(guess_content_type("photo.jpeg"), "image/jpeg");
        assert_eq!(guess_content_type("README"), "application/octet-stream");
    }

    #[test]
    fn attachment_paths_keep_the_hash_colon() {
        let path = attachment_path("uploads", "sha256:abc");
        assert_eq!(path, "/v1/namespaces/uploads/attachments/sha256:abc");
    }

    #[test]
    fn transcribe_requires_an_agent_id() {
        let cmd = build_attachment_command();
        assert!(cmd
            .try_get_matches_from(["attachment", "transcribe", "ns", "sha256:a"])
            .is_err());
    }

    #[test]
    fn job_body_carries_the_memory_fields() {
        let m = build_attachment_command()
            .try_get_matches_from([
                "attachment",
                "transcribe",
                "uploads",
                "sha256:a",
                "--agent-id",
                "bot",
                "--tag",
                "voice",
                "--tag",
                "call",
                "--importance",
                "0.7",
                "--lang",
                "de",
            ])
            .unwrap();
        let (_, sub) = m.subcommand().unwrap();
        let body = job_body(sub, "transcribe");
        assert_eq!(body["agent_id"], "bot");
        assert_eq!(body["tags"], json!(["voice", "call"]));
        assert_eq!(body["importance"], json!(0.7));
        assert_eq!(body["lang"], "de");
        assert!(body.get("content").is_none());
    }

    #[test]
    fn index_body_carries_the_caption() {
        let m = build_attachment_command()
            .try_get_matches_from([
                "attachment",
                "index",
                "uploads",
                "sha256:a",
                "--agent-id",
                "bot",
                "--content",
                "page 3",
            ])
            .unwrap();
        let (_, sub) = m.subcommand().unwrap();
        let body = job_body(sub, "index");
        assert_eq!(body["content"], "page 3");
    }

    #[test]
    fn failed_job_becomes_an_http_error() {
        let job = json!({
            "status": "Failed",
            "message": "the audio holds no speech",
            "error": {"status": 400, "code": "INVALID_REQUEST"}
        });
        let err = job_failure(&job);
        let api = err.downcast_ref::<ApiHttpError>().unwrap();
        assert_eq!(api.status, 400);
        assert_eq!(api.code.as_deref(), Some("INVALID_REQUEST"));
        assert!(api.message.contains("no speech"));
    }

    #[test]
    fn failed_job_without_error_object_is_a_500() {
        let err = job_failure(&json!({"status": "Cancelled"}));
        let api = err.downcast_ref::<ApiHttpError>().unwrap();
        assert_eq!(api.status, 500);
    }
}
