//! Admin commands for cluster management, caching, backups, and configuration

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde_json::Value;

use crate::output;
use crate::OutputFormat;

/// Helper: build a reqwest client and make a GET request, returning JSON Value
async fn admin_get(url: &str, path: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}{}", url, path))
        .send()
        .await
        .with_context(|| format!("Failed to GET {}", path))?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, body);
    }
    serde_json::from_str(&body).with_context(|| "Failed to parse response JSON")
}

/// Helper: make a POST request with optional JSON body, returning JSON Value
async fn admin_post(url: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let client = reqwest::Client::new();
    let mut req = client.post(format!("{}{}", url, path));
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("Failed to POST {}", path))?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    if text.is_empty() {
        Ok(Value::Object(serde_json::Map::new()))
    } else {
        serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
    }
}

/// Helper: make a DELETE request, returning JSON Value
async fn admin_delete(url: &str, path: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}{}", url, path))
        .send()
        .await
        .with_context(|| format!("Failed to DELETE {}", path))?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    if text.is_empty() {
        Ok(Value::Object(serde_json::Map::new()))
    } else {
        serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
    }
}

/// Helper: make a PUT request with JSON body, returning JSON Value
async fn admin_put(url: &str, path: &str, body: &Value) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .put(format!("{}{}", url, path))
        .json(body)
        .send()
        .await
        .with_context(|| format!("Failed to PUT {}", path))?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    if text.is_empty() {
        Ok(Value::Object(serde_json::Map::new()))
    } else {
        serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
    }
}

fn print_value(value: &Value, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
        }
        OutputFormat::Compact => {
            println!("{}", serde_json::to_string(value).unwrap_or_default());
        }
        OutputFormat::Table => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
        }
    }
}

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    match matches.subcommand() {
        Some(("cluster-status", _sub)) => {
            let result = admin_get(url, "/admin/cluster/status").await?;
            output::info("Cluster Status");
            print_value(&result, format);
        }

        Some(("cluster-nodes", _sub)) => {
            let result = admin_get(url, "/admin/cluster/nodes").await?;
            output::info("Cluster Nodes");
            print_value(&result, format);
        }

        Some(("optimize", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let result = admin_post(
                url,
                &format!("/admin/namespaces/{}/optimize", namespace),
                None,
            )
            .await?;
            output::success(&format!("Namespace '{}' optimization started", namespace));
            print_value(&result, format);
        }

        Some(("index-stats", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let result = admin_get(
                url,
                &format!("/admin/indexes/stats?namespace={}", namespace),
            )
            .await?;
            output::info(&format!("Index stats for '{}'", namespace));
            print_value(&result, format);
        }

        Some(("rebuild-indexes", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let body = serde_json::json!({ "namespace": namespace });
            let result = admin_post(url, "/admin/indexes/rebuild", Some(&body)).await?;
            output::success(&format!("Index rebuild started for '{}'", namespace));
            print_value(&result, format);
        }

        Some(("cache-stats", _sub)) => {
            let result = admin_get(url, "/admin/cache/stats").await?;
            output::info("Cache Statistics");
            print_value(&result, format);
        }

        Some(("cache-clear", sub)) => {
            let namespace = sub.get_one::<String>("namespace");
            let body = match namespace {
                Some(ns) => serde_json::json!({ "namespace": ns }),
                None => serde_json::json!({}),
            };
            let result = admin_post(url, "/admin/cache/clear", Some(&body)).await?;
            if let Some(ns) = namespace {
                output::success(&format!("Cache cleared for namespace '{}'", ns));
            } else {
                output::success("Cache cleared for all namespaces");
            }
            print_value(&result, format);
        }

        Some(("config-get", _sub)) => {
            let result = admin_get(url, "/admin/config").await?;
            output::info("Server Configuration");
            print_value(&result, format);
        }

        Some(("config-set", sub)) => {
            let key = sub.get_one::<String>("key").unwrap();
            let value = sub.get_one::<String>("value").unwrap();

            // Try to parse value as JSON first, fall back to string
            let json_value: Value =
                serde_json::from_str(value).unwrap_or(Value::String(value.clone()));
            let body = serde_json::json!({ key: json_value });
            let result = admin_put(url, "/admin/config", &body).await?;
            output::success(&format!("Configuration updated: {} = {}", key, value));
            print_value(&result, format);
        }

        Some(("quotas-get", _sub)) => {
            let result = admin_get(url, "/admin/quotas").await?;
            output::info("Namespace Quotas");
            print_value(&result, format);
        }

        Some(("quotas-set", sub)) => {
            let data = sub.get_one::<String>("data").unwrap();
            let body: Value =
                serde_json::from_str(data).with_context(|| "Invalid JSON for --data")?;
            let result = admin_put(url, "/admin/quotas", &body).await?;
            output::success("Quotas updated");
            print_value(&result, format);
        }

        Some(("slow-queries", sub)) => {
            let limit = sub.get_one::<u32>("limit").copied().unwrap_or(20);
            let min_duration = sub.get_one::<f64>("min-duration");

            let mut path = format!("/admin/slow-queries?limit={}", limit);
            if let Some(dur) = min_duration {
                path.push_str(&format!("&min_duration_ms={}", dur));
            }

            let result = admin_get(url, &path).await?;
            output::info("Slow Queries");
            print_value(&result, format);
        }

        Some(("backup-create", sub)) => {
            let no_data = sub.get_flag("no-data");
            let body = serde_json::json!({
                "include_data": !no_data
            });
            let result = admin_post(url, "/admin/backups", Some(&body)).await?;
            output::success("Backup created");
            print_value(&result, format);
        }

        Some(("backup-list", _sub)) => {
            let result = admin_get(url, "/admin/backups").await?;
            output::info("Backups");
            print_value(&result, format);
        }

        Some(("backup-restore", sub)) => {
            let backup_id = sub.get_one::<String>("backup_id").unwrap();
            let body = serde_json::json!({ "backup_id": backup_id });
            let result = admin_post(url, "/admin/backups/restore", Some(&body)).await?;
            output::success(&format!("Restore started from backup '{}'", backup_id));
            print_value(&result, format);
        }

        Some(("backup-delete", sub)) => {
            let backup_id = sub.get_one::<String>("backup_id").unwrap();
            let result = admin_delete(url, &format!("/admin/backups/{}", backup_id)).await?;
            output::success(&format!("Backup '{}' deleted", backup_id));
            print_value(&result, format);
        }

        Some(("configure-ttl", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let ttl_seconds = sub.get_one::<u64>("ttl-seconds").unwrap();
            let strategy = sub.get_one::<String>("strategy");

            let mut body = serde_json::json!({
                "ttl_seconds": ttl_seconds,
            });
            if let Some(s) = strategy {
                body.as_object_mut()
                    .unwrap()
                    .insert("strategy".to_string(), Value::String(s.clone()));
            }
            let result =
                admin_put(url, &format!("/admin/namespaces/{}/ttl", namespace), &body).await?;
            output::success(&format!(
                "TTL configured for '{}': {} seconds",
                namespace, ttl_seconds
            ));
            print_value(&result, format);
        }

        _ => {
            output::error("Unknown admin subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
