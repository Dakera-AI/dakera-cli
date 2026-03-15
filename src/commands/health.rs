//! Health check command

use anyhow::Result;
use dakera_client::DakeraClient;
use nu_ansi_term::{Color, Style};

use crate::output;
use crate::OutputFormat;

pub async fn execute(url: &str, detailed: bool, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    // Check basic health
    let health = client.health().await;

    match health {
        Ok(h) => {
            if detailed {
                // Get additional health information
                let ready = client.ready().await.ok();
                let live = client.live().await.unwrap_or(false);
                let diagnostics = client.diagnostics().await.ok();

                let green = Style::new().fg(Color::Green);
                let red = Style::new().fg(Color::Red);
                let yellow = Style::new().fg(Color::Yellow);
                let cyan = Style::new().fg(Color::Cyan).bold();

                let pairs = [
                    (
                        "Status",
                        if h.healthy {
                            green.paint("Healthy").to_string()
                        } else {
                            red.paint("Unhealthy").to_string()
                        },
                    ),
                    (
                        "Live",
                        if live {
                            green.paint("Yes").to_string()
                        } else {
                            red.paint("No").to_string()
                        },
                    ),
                    (
                        "Ready",
                        ready
                            .as_ref()
                            .map(|r| {
                                if r.ready {
                                    green.paint("Yes").to_string()
                                } else {
                                    yellow.paint("No").to_string()
                                }
                            })
                            .unwrap_or_else(|| "Unknown".to_string()),
                    ),
                    (
                        "Version",
                        h.version.unwrap_or_else(|| "Unknown".to_string()),
                    ),
                    (
                        "Uptime",
                        h.uptime_seconds
                            .map(format_duration)
                            .unwrap_or_else(|| "Unknown".to_string()),
                    ),
                ];

                output::print_kv(
                    &pairs
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .collect::<Vec<_>>(),
                    format,
                );

                if let Some(diag) = diagnostics {
                    println!();
                    println!("{}", cyan.paint("System Diagnostics:"));
                    println!(
                        "  Memory Used: {} MB",
                        diag.resources.memory_bytes / 1024 / 1024
                    );
                    println!("  Threads: {}", diag.resources.thread_count);
                    println!("  Open FDs: {}", diag.resources.open_fds);
                    println!("  Active Jobs: {}", diag.active_jobs);
                }
            } else if h.healthy {
                output::success(&format!("Server at {} is healthy", url));
                if let Some(v) = h.version {
                    println!("  Version: {}", v);
                }
            } else {
                output::error(&format!("Server at {} is unhealthy", url));
            }
        }
        Err(e) => {
            output::error(&format!("Failed to connect to server at {}: {}", url, e));
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
