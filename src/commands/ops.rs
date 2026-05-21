//! Operations and maintenance commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::{reqwest, CompactionRequest, DakeraClient, OpsStats};
use nu_ansi_term::{Color, Style};
use serde::Serialize;

use crate::context::Context;
use crate::output;

#[derive(Debug, Serialize)]
pub struct JobRow {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub progress: String,
    pub created_at: String,
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("stats", _)) => {
            let t = ctx.log_request("GET", "/ops/stats");
            let stats: OpsStats = client.ops_stats().await?;
            ctx.log_response(t, "200 OK");

            let state_label = match stats.state.as_str() {
                "healthy" => format!("{} (healthy)", stats.state),
                "degraded" => format!("{} (degraded — check storage)", stats.state),
                other => other.to_string(),
            };

            let pairs = [
                ("Server Version", stats.version.clone()),
                ("State", state_label),
                ("Total Vectors", stats.total_vectors.to_string()),
                ("Namespaces", stats.namespace_count.to_string()),
                ("Uptime", format_duration(stats.uptime_seconds)),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                ctx.format,
            );
        }

        Some(("diagnostics", _)) => {
            let t = ctx.log_request("GET", "/ops/diagnostics");
            let diag = client.diagnostics().await?;
            ctx.log_response(t, "200 OK");

            let pairs = [
                ("Server Version", diag.system.version.clone()),
                ("Rust Version", diag.system.rust_version.clone()),
                ("Uptime", format_duration(diag.system.uptime_seconds)),
                ("PID", diag.system.pid.to_string()),
                (
                    "Memory Used",
                    format!("{} MB", diag.resources.memory_bytes / 1024 / 1024),
                ),
                ("Threads", diag.resources.thread_count.to_string()),
                ("Open FDs", diag.resources.open_fds.to_string()),
                ("Active Jobs", diag.active_jobs.to_string()),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                ctx.format,
            );

            let cyan = Style::new().fg(Color::Cyan).bold();
            let green = Style::new().fg(Color::Green);
            let red = Style::new().fg(Color::Red);

            println!();
            println!("{}", cyan.paint("Component Health:"));
            println!(
                "  Storage: {} - {}",
                if diag.components.storage.healthy {
                    green.paint("OK")
                } else {
                    red.paint("FAIL")
                },
                diag.components.storage.message
            );
            println!(
                "  Search Engine: {} - {}",
                if diag.components.search_engine.healthy {
                    green.paint("OK")
                } else {
                    red.paint("FAIL")
                },
                diag.components.search_engine.message
            );
            println!(
                "  Cache: {} - {}",
                if diag.components.cache.healthy {
                    green.paint("OK")
                } else {
                    red.paint("FAIL")
                },
                diag.components.cache.message
            );
            println!(
                "  gRPC: {} - {}",
                if diag.components.grpc.healthy {
                    green.paint("OK")
                } else {
                    red.paint("FAIL")
                },
                diag.components.grpc.message
            );
        }

        Some(("jobs", _)) => {
            let t = ctx.log_request("GET", "/ops/jobs");
            let jobs = client.list_jobs().await?;
            ctx.log_response(t, "200 OK");

            if jobs.is_empty() {
                output::info("No background jobs");
            } else {
                let rows: Vec<JobRow> = jobs
                    .into_iter()
                    .map(|j| JobRow {
                        id: j.id,
                        job_type: j.job_type,
                        status: j.status,
                        progress: format!("{}%", j.progress),
                        created_at: format_timestamp(j.created_at),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("job", sub_matches)) => {
            let id = sub_matches.get_one::<String>("id").unwrap();
            let t = ctx.log_request("GET", &format!("/ops/jobs/{}", id));
            let job = client.get_job(id).await?;
            ctx.log_response(t, "200 OK");

            match job {
                Some(j) => {
                    let pairs = [
                        ("ID", j.id),
                        ("Type", j.job_type),
                        ("Status", j.status),
                        ("Progress", format!("{}%", j.progress)),
                        ("Created", format_timestamp(j.created_at)),
                        ("Message", j.message.unwrap_or_else(|| "-".to_string())),
                    ];
                    output::print_kv(
                        &pairs
                            .iter()
                            .map(|(k, v)| (*k, v.clone()))
                            .collect::<Vec<_>>(),
                        ctx.format,
                    );
                }
                None => {
                    output::error(&format!("Job '{}' not found", id));
                    std::process::exit(1);
                }
            }
        }

        Some(("compact", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").cloned();
            let force = sub_matches.get_flag("force");

            output::info("Triggering compaction...");

            let request = CompactionRequest {
                namespace: namespace.clone(),
                force,
            };

            let t = ctx.log_request("POST", "/ops/compact");
            let response = client.compact(request).await?;
            ctx.log_response(t, "200 OK");

            output::success(&format!("Compaction started (job: {})", response.job_id));
            output::info(&response.message);
            if let Some(ns) = namespace {
                output::info(&format!("Target namespace: {}", ns));
            } else {
                output::info("Compacting all namespaces");
            }
        }

        Some(("shutdown", sub_matches)) => {
            let yes = sub_matches.get_flag("yes");
            let dry_run = sub_matches.get_flag("dry-run");

            if dry_run {
                output::info(
                    "[dry-run] Would send graceful shutdown request to the server (no action taken)",
                );
                output::info("[dry-run] Re-run without --dry-run to initiate the shutdown");
                return Ok(());
            }

            if !yes {
                output::warning("This will gracefully shutdown the Dakera server");
                print!("Are you sure? [y/N]: ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    output::info("Shutdown cancelled");
                    return Ok(());
                }
            }

            output::info("Requesting graceful shutdown...");
            let t = ctx.log_request("POST", "/ops/shutdown");
            client.shutdown().await?;
            ctx.log_response(t, "200 OK");
            output::success("Shutdown request sent");
        }

        Some(("metrics", _)) => {
            let path = "/metrics";
            let t = ctx.log_request("GET", path);
            let metrics_url = format!("{}{}", ctx.url, path);
            let response = reqwest::get(&metrics_url).await?;

            if response.status().is_success() {
                ctx.log_response(t, "200 OK");
                let text = response.text().await?;
                println!("{}", text);
            } else {
                ctx.log_response(t, "ERR");
                output::error("Failed to fetch metrics. Is the metrics endpoint enabled?");
                std::process::exit(1);
            }
        }

        _ => {
            output::error("Unknown ops subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_ops_command;

    #[test]
    fn ops_diagnostics_subcommand_recognized() {
        build_ops_command()
            .try_get_matches_from(["ops", "diagnostics"])
            .expect("ops diagnostics should parse");
    }

    #[test]
    fn ops_metrics_subcommand_recognized() {
        build_ops_command()
            .try_get_matches_from(["ops", "metrics"])
            .expect("ops metrics should parse");
    }

    #[test]
    fn ops_job_requires_id() {
        assert!(
            build_ops_command()
                .try_get_matches_from(["ops", "job"])
                .is_err(),
            "ops job without id should fail"
        );
    }

    #[test]
    fn ops_compact_with_namespace_flag() {
        let m = build_ops_command()
            .try_get_matches_from(["ops", "compact", "--namespace", "my-ns"])
            .expect("ops compact with --namespace should parse");
        let sub = m.subcommand_matches("compact").unwrap();
        assert_eq!(sub.get_one::<String>("namespace").unwrap(), "my-ns");
    }
}

fn format_duration(seconds: u64) -> String {
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

fn format_timestamp(ts: u64) -> String {
    let secs_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().saturating_sub(ts))
        .unwrap_or(0);

    if secs_ago < 60 {
        format!("{}s ago", secs_ago)
    } else if secs_ago < 3600 {
        format!("{}m ago", secs_ago / 60)
    } else if secs_ago < 86400 {
        format!("{}h ago", secs_ago / 3600)
    } else {
        format!("{}d ago", secs_ago / 86400)
    }
}
