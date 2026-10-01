//! Health commands: `dk health [--detailed]`, `dk health ready`, `dk health live`.
//!
//! These read the server's JSON directly instead of going through the SDK:
//! while a v0.12 server loads its models it answers `/health` and
//! `/health/ready` with `503` and a `Retry-After` header, which an SDK that
//! only looks at the body can mistake for a healthy server.

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use nu_ansi_term::{Color, Style};
use serde_json::Value;

use crate::api::{self, bool_of, ptr_u64, str_of, u64_of, Probe};
use crate::context::Context;
use crate::output;
use crate::OutputFormat;

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("ready", _)) => ready(ctx).await,
        Some(("live", _)) => live(ctx).await,
        _ => {
            if matches.get_flag("detailed") {
                detailed(ctx).await
            } else {
                basic(ctx).await
            }
        }
    }
}

fn is_json(ctx: &Context) -> bool {
    matches!(ctx.format, OutputFormat::Json | OutputFormat::Compact)
}

/// The status word of a `/health` body: v0.12 sends `status`
/// (`healthy` / `degraded`), earlier servers a `healthy` boolean.
fn status_of(h: &Value) -> String {
    if let Some(s) = str_of(h, "status") {
        return s.to_string();
    }
    if bool_of(h, "healthy") == Some(true) {
        "healthy".to_string()
    } else {
        "unhealthy".to_string()
    }
}

/// Print `  component: reason` lines under a title, when there are any.
fn print_entries(title: &str, list: Option<&Value>) {
    let Some(list) = list else {
        return;
    };
    let Some(items) = list.as_array() else {
        return;
    };
    if items.is_empty() {
        return;
    }
    println!("  {title}:");
    for item in items {
        let component = str_of(item, "component").unwrap_or("?");
        let reason = str_of(item, "reason").unwrap_or("");
        println!("    {component}: {reason}");
    }
}

/// One line for the `embed_migration` summary of `/health` (or the body of
/// `GET /admin/reembed/migration`).
pub fn migration_line(m: &Value) -> String {
    let state = str_of(m, "state").unwrap_or("unknown");
    let remaining = u64_of(m, "remaining").unwrap_or(0);
    let done = u64_of(m, "reembedded").unwrap_or(0);
    let mut line = format!("Embed migration: {state} ({remaining} remaining, {done} re-embedded)");
    if let Some(eta) = u64_of(m, "eta_secs") {
        line.push_str(&format!(", ETA {}", format_duration(eta)));
    }
    line
}

async fn basic(ctx: &Context) -> Result<()> {
    let probe = api::probe(ctx, "/health").await?;
    if !probe.is_success() {
        return Err(anyhow::Error::new(probe.into_error()));
    }
    let h = probe.body;
    let status = status_of(&h);
    if status != "healthy" && status != "degraded" {
        anyhow::bail!("Server at {} is unhealthy (status: {status})", ctx.url);
    }
    if is_json(ctx) {
        output::print_item(&h, ctx.format);
        return Ok(());
    }
    if status == "healthy" {
        output::success(&format!("Server at {} is healthy", ctx.url));
    } else {
        output::warning(&format!("Server at {} is degraded", ctx.url));
    }
    if let Some(v) = str_of(&h, "version") {
        println!("  Version: {v}");
    }
    print_health_details(&h);
    Ok(())
}

/// The advice, degraded components, configuration warnings and embed
/// migration of a `/health` body.
fn print_health_details(h: &Value) {
    if let Some(advice) = str_of(h, "advice") {
        println!("  Advice: {advice}");
    }
    print_entries("Degraded components", h.get("degraded"));
    print_entries("Configuration warnings", h.get("config_warnings"));
    if let Some(m) = h.get("embed_migration") {
        println!("  {}", migration_line(m));
    }
}

async fn ready(ctx: &Context) -> Result<()> {
    let probe = api::probe(ctx, "/health/ready").await?;
    let reported = bool_of(&probe.body, "ready");
    let is_ready = probe.is_success() && reported.unwrap_or(true);
    if is_json(ctx) {
        output::print_item(&probe.body, ctx.format);
    } else if is_ready {
        output::success(&format!("Server at {} is ready", ctx.url));
        print_checks(&probe.body);
    } else {
        output::warning(&format!("Server at {} is not ready", ctx.url));
        print_not_ready(&probe);
    }
    if is_ready {
        return Ok(());
    }
    if probe.is_success() {
        anyhow::bail!("Server at {} reports it is not ready", ctx.url);
    }
    let has_ready_body = reported.is_some();
    let mut err = probe.into_error();
    if has_ready_body {
        err.message = "the server is not ready".to_string();
    }
    Err(anyhow::Error::new(err))
}

fn print_checks(body: &Value) {
    let Some(checks) = body.get("checks") else {
        return;
    };
    let Some(checks) = checks.as_object() else {
        return;
    };
    for (name, check) in checks {
        let status = str_of(check, "status").unwrap_or("unknown");
        match str_of(check, "message") {
            Some(message) => println!("  {name}: {status} ({message})"),
            None => println!("  {name}: {status}"),
        }
    }
}

fn print_not_ready(probe: &Probe) {
    let body = &probe.body;
    if let Some(reason) = str_of(body, "reason") {
        println!("  Reason: {reason}");
    }
    if let Some(downloads) = body.get("downloads") {
        for d in downloads.as_array().into_iter().flatten() {
            let repo = str_of(d, "repo").unwrap_or("?");
            let file = str_of(d, "file").unwrap_or("?");
            let received = u64_of(d, "received_bytes").unwrap_or(0);
            match u64_of(d, "total_bytes") {
                Some(total) => println!("  Downloading {repo}/{file}: {received} / {total} bytes"),
                None => println!("  Downloading {repo}/{file}: {received} bytes"),
            }
        }
    }
    print_checks(body);
}

async fn live(ctx: &Context) -> Result<()> {
    let probe = api::probe(ctx, "/health/live").await?;
    if !probe.is_success() {
        return Err(anyhow::Error::new(probe.into_error()));
    }
    if is_json(ctx) {
        output::print_item(&probe.body, ctx.format);
        return Ok(());
    }
    output::success(&format!("Server at {} is alive", ctx.url));
    if let Some(v) = str_of(&probe.body, "version") {
        println!("  Version: {v}");
    }
    if let Some(secs) = u64_of(&probe.body, "uptime_seconds") {
        println!("  Uptime: {}", format_duration(secs));
    }
    if bool_of(&probe.body, "starting") == Some(true) {
        println!("  Still starting: `dk health ready` says when it is ready");
    }
    Ok(())
}

async fn detailed(ctx: &Context) -> Result<()> {
    let probe = api::probe(ctx, "/health").await?;
    if !probe.is_success() {
        return Err(anyhow::Error::new(probe.into_error()));
    }
    let h = probe.body;
    let ready = api::probe(ctx, "/health/ready").await.ok();
    let live = api::probe(ctx, "/health/live").await.ok();
    let diag = api::request_json_logged(ctx, Method::GET, "/ops/diagnostics", None).await;
    let diagnostics = diag.ok();

    if is_json(ctx) {
        let ready_body = ready.as_ref().map(|p| p.body.clone());
        let live_body = live.as_ref().map(|p| p.body.clone());
        let combined = serde_json::json!({
            "health": h,
            "ready": ready_body,
            "live": live_body,
            "diagnostics": diagnostics,
        });
        output::print_item(&combined, ctx.format);
        return Ok(());
    }

    let green = Style::new().fg(Color::Green);
    let red = Style::new().fg(Color::Red);
    let yellow = Style::new().fg(Color::Yellow);
    let cyan = Style::new().fg(Color::Cyan).bold();

    let status = status_of(&h);
    let status_cell = match status.as_str() {
        "healthy" => green.paint("Healthy").to_string(),
        "degraded" => yellow.paint("Degraded").to_string(),
        _ => red.paint("Unhealthy").to_string(),
    };
    let live_cell = match &live {
        Some(p) if p.is_success() => green.paint("Yes").to_string(),
        Some(_) => red.paint("No").to_string(),
        None => "Unknown".to_string(),
    };
    let ready_cell = match &ready {
        Some(p) if p.is_success() => green.paint("Yes").to_string(),
        Some(_) => yellow.paint("No").to_string(),
        None => "Unknown".to_string(),
    };
    let version = str_of(&h, "version").unwrap_or("Unknown").to_string();
    let mut uptime = "Unknown".to_string();
    if let Some(p) = &live {
        if let Some(secs) = u64_of(&p.body, "uptime_seconds") {
            uptime = format_duration(secs);
        }
    }

    let mut pairs: Vec<(&str, String)> = vec![
        ("Status", status_cell),
        ("Live", live_cell),
        ("Ready", ready_cell),
        ("Version", version),
        ("Uptime", uptime),
    ];
    if let Some(sha) = str_of(&h, "build_sha") {
        pairs.push(("Build", sha.to_string()));
    }
    output::print_kv(&pairs, ctx.format);

    print_health_details(&h);
    if let Some(p) = &ready {
        if !p.is_success() {
            print_not_ready(p);
        }
    }

    if let Some(diag) = diagnostics {
        println!();
        println!("{}", cyan.paint("System Diagnostics:"));
        if let Some(bytes) = ptr_u64(&diag, "/resources/memory_bytes") {
            println!("  Memory Used: {} MB", bytes / 1024 / 1024);
        }
        if let Some(n) = ptr_u64(&diag, "/resources/thread_count") {
            println!("  Threads: {n}");
        }
        if let Some(n) = ptr_u64(&diag, "/resources/open_fds") {
            println!("  Open FDs: {n}");
        }
        if let Some(n) = u64_of(&diag, "active_jobs") {
            println!("  Active Jobs: {n}");
        }
    }

    Ok(())
}

pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3599), "59m 59s");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(3600), "1h 0m");
        assert_eq!(format_duration(3660), "1h 1m");
        assert_eq!(format_duration(86399), "23h 59m");
    }

    #[test]
    fn format_duration_days_and_hours() {
        assert_eq!(format_duration(86400), "1d 0h");
        assert_eq!(format_duration(90000), "1d 1h");
        assert_eq!(format_duration(172800), "2d 0h");
    }

    #[test]
    fn status_of_reads_v012_status_word() {
        assert_eq!(status_of(&json!({"status": "degraded"})), "degraded");
        assert_eq!(status_of(&json!({"status": "healthy"})), "healthy");
    }

    #[test]
    fn status_of_falls_back_to_the_boolean() {
        assert_eq!(status_of(&json!({"healthy": true})), "healthy");
        assert_eq!(status_of(&json!({"healthy": false})), "unhealthy");
        assert_eq!(status_of(&json!({})), "unhealthy");
    }

    #[test]
    fn migration_line_with_eta() {
        let m = json!({"state": "running", "remaining": 120, "reembedded": 30, "eta_secs": 3700});
        let line = migration_line(&m);
        assert!(line.contains("running"));
        assert!(line.contains("120 remaining"));
        assert!(line.contains("30 re-embedded"));
        assert!(line.contains("ETA 1h 1m"));
    }

    #[test]
    fn migration_line_without_eta() {
        let line = migration_line(&json!({"state": "complete"}));
        assert!(line.contains("complete"));
        assert!(!line.contains("ETA"));
    }
}
