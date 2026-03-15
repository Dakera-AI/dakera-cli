//! Analytics commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;

use crate::output;
use crate::OutputFormat;

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("overview", sub_matches)) => {
            let period = sub_matches.get_one::<String>("period").map(|s| s.as_str());
            let namespace = sub_matches
                .get_one::<String>("namespace")
                .map(|s| s.as_str());

            let overview = client.analytics_overview(period, namespace).await?;

            let pairs = [
                ("Total Queries", overview.total_queries.to_string()),
                ("Avg Latency", format!("{:.2}ms", overview.avg_latency_ms)),
                ("P95 Latency", format!("{:.2}ms", overview.p95_latency_ms)),
                ("P99 Latency", format!("{:.2}ms", overview.p99_latency_ms)),
                ("Queries/sec", format!("{:.2}", overview.queries_per_second)),
                ("Error Rate", format!("{:.4}", overview.error_rate)),
                (
                    "Cache Hit Rate",
                    format!("{:.2}%", overview.cache_hit_rate * 100.0),
                ),
                ("Storage Used", format_bytes(overview.storage_used_bytes)),
                ("Total Vectors", overview.total_vectors.to_string()),
                ("Total Namespaces", overview.total_namespaces.to_string()),
                ("Uptime", format_duration(overview.uptime_seconds)),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                format,
            );
        }

        Some(("latency", sub_matches)) => {
            let period = sub_matches.get_one::<String>("period").map(|s| s.as_str());
            let namespace = sub_matches
                .get_one::<String>("namespace")
                .map(|s| s.as_str());

            let latency = client.analytics_latency(period, namespace).await?;

            let pairs = [
                ("Period", latency.period.clone()),
                ("Avg", format!("{:.2}ms", latency.avg_ms)),
                ("P50", format!("{:.2}ms", latency.p50_ms)),
                ("P95", format!("{:.2}ms", latency.p95_ms)),
                ("P99", format!("{:.2}ms", latency.p99_ms)),
                ("Max", format!("{:.2}ms", latency.max_ms)),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                format,
            );

            if !latency.by_operation.is_empty() {
                println!();
                output::info("By operation:");
                for (op, stats) in &latency.by_operation {
                    println!(
                        "  {}: avg={:.2}ms p95={:.2}ms count={}",
                        op, stats.avg_ms, stats.p95_ms, stats.count
                    );
                }
            }
        }

        Some(("throughput", sub_matches)) => {
            let period = sub_matches.get_one::<String>("period").map(|s| s.as_str());
            let namespace = sub_matches
                .get_one::<String>("namespace")
                .map(|s| s.as_str());

            let throughput = client.analytics_throughput(period, namespace).await?;

            let pairs = [
                ("Period", throughput.period.clone()),
                ("Total Operations", throughput.total_operations.to_string()),
                (
                    "Operations/sec",
                    format!("{:.2}", throughput.operations_per_second),
                ),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                format,
            );

            if !throughput.by_operation.is_empty() {
                println!();
                output::info("By operation:");
                for (op, count) in &throughput.by_operation {
                    println!("  {}: {}", op, count);
                }
            }
        }

        Some(("storage", sub_matches)) => {
            let namespace = sub_matches
                .get_one::<String>("namespace")
                .map(|s| s.as_str());

            let storage = client.analytics_storage(namespace).await?;

            let pairs = [
                ("Total", format_bytes(storage.total_bytes)),
                ("Index", format_bytes(storage.index_bytes)),
                ("Data", format_bytes(storage.data_bytes)),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                format,
            );

            if !storage.by_namespace.is_empty() {
                println!();
                output::info("By namespace:");
                for (ns, stats) in &storage.by_namespace {
                    println!(
                        "  {}: {} ({} vectors)",
                        ns,
                        format_bytes(stats.bytes),
                        stats.vector_count
                    );
                }
            }
        }

        _ => {
            output::error("Unknown analytics subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Format bytes into human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format seconds into human-readable duration
fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}
