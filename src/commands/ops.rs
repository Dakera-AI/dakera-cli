//! Operations and maintenance commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::{reqwest, CompactionRequest, DakeraClient};
use nu_ansi_term::{Color, Style};
use serde::Serialize;

use crate::output;
use crate::OutputFormat;

#[derive(Debug, Serialize)]
pub struct JobRow {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub progress: String,
    pub created_at: String,
}

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("diagnostics", _)) => {
            let diag = client.diagnostics().await?;

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
                format,
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
            let jobs = client.list_jobs().await?;

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
                output::print_data(&rows, format);
            }
        }

        Some(("job", sub_matches)) => {
            let id = sub_matches.get_one::<String>("id").unwrap();
            let job = client.get_job(id).await?;

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
                        format,
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

            let response = client.compact(request).await?;

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
            client.shutdown().await?;
            output::success("Shutdown request sent");
        }

        Some(("metrics", _)) => {
            // Fetch Prometheus metrics
            let metrics_url = format!("{}/metrics", url);
            let response = reqwest::get(&metrics_url).await?;

            if response.status().is_success() {
                let text = response.text().await?;
                println!("{}", text);
            } else {
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
    // Simple timestamp formatting (seconds since epoch)
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
