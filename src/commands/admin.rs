//! Admin commands for cluster management, caching, backups, and configuration

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::output;

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

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("cluster-status", _sub)) => {
            let path = "/admin/cluster/status";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Cluster Status");
            output::print_item(&result?, ctx.format);
        }

        Some(("cluster-nodes", _sub)) => {
            let path = "/admin/cluster/nodes";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Cluster Nodes");
            output::print_item(&result?, ctx.format);
        }

        Some(("optimize", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = format!("/admin/namespaces/{}/optimize", namespace);
            let t = ctx.log_request("POST", &path);
            let result = admin_post(&ctx.url, &path, None).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("Namespace '{}' optimization started", namespace));
            output::print_item(&result?, ctx.format);
        }

        Some(("index-stats", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = format!("/admin/indexes/stats?namespace={}", namespace);
            let t = ctx.log_request("GET", &path);
            let result = admin_get(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info(&format!("Index stats for '{}'", namespace));
            output::print_item(&result?, ctx.format);
        }

        Some(("rebuild-indexes", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = "/admin/indexes/rebuild";
            let body = serde_json::json!({ "namespace": namespace });
            let t = ctx.log_request("POST", path);
            let result = admin_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("Index rebuild started for '{}'", namespace));
            output::print_item(&result?, ctx.format);
        }

        Some(("cache-stats", _sub)) => {
            let path = "/admin/cache/stats";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Cache Statistics");
            output::print_item(&result?, ctx.format);
        }

        Some(("cache-clear", sub)) => {
            let namespace = sub.get_one::<String>("namespace");
            let body = match namespace {
                Some(ns) => serde_json::json!({ "namespace": ns }),
                None => serde_json::json!({}),
            };
            let path = "/admin/cache/clear";
            let t = ctx.log_request("POST", path);
            let result = admin_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            if let Some(ns) = namespace {
                output::success(&format!("Cache cleared for namespace '{}'", ns));
            } else {
                output::success("Cache cleared for all namespaces");
            }
            output::print_item(&result?, ctx.format);
        }

        Some(("config-get", _sub)) => {
            let path = "/admin/config";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Server Configuration");
            output::print_item(&result?, ctx.format);
        }

        Some(("config-set", sub)) => {
            let key = sub.get_one::<String>("key").unwrap();
            let value = sub.get_one::<String>("value").unwrap();
            let json_value: Value =
                serde_json::from_str(value).unwrap_or(Value::String(value.clone()));
            let body = serde_json::json!({ key: json_value });
            let path = "/admin/config";
            let t = ctx.log_request("PUT", path);
            let result = admin_put(&ctx.url, path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("Configuration updated: {} = {}", key, value));
            output::print_item(&result?, ctx.format);
        }

        Some(("quotas-get", _sub)) => {
            let path = "/admin/quotas";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Namespace Quotas");
            output::print_item(&result?, ctx.format);
        }

        Some(("quotas-set", sub)) => {
            let data = sub.get_one::<String>("data").unwrap();
            let body: Value =
                serde_json::from_str(data).with_context(|| "Invalid JSON for --data")?;
            let path = "/admin/quotas";
            let t = ctx.log_request("PUT", path);
            let result = admin_put(&ctx.url, path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success("Quotas updated");
            output::print_item(&result?, ctx.format);
        }

        Some(("slow-queries", sub)) => {
            let limit = sub.get_one::<u32>("limit").copied().unwrap_or(20);
            let min_duration = sub.get_one::<f64>("min-duration");
            let mut path = format!("/admin/slow-queries?limit={}", limit);
            if let Some(dur) = min_duration {
                path.push_str(&format!("&min_duration_ms={}", dur));
            }
            let t = ctx.log_request("GET", &path);
            let result = admin_get(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Slow Queries");
            output::print_item(&result?, ctx.format);
        }

        Some(("backup-create", sub)) => {
            let no_data = sub.get_flag("no-data");
            let body = serde_json::json!({ "include_data": !no_data });
            let path = "/admin/backups";
            let t = ctx.log_request("POST", path);
            let result = admin_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success("Backup created");
            output::print_item(&result?, ctx.format);
        }

        Some(("backup-list", _sub)) => {
            let path = "/admin/backups";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("Backups");
            output::print_item(&result?, ctx.format);
        }

        Some(("backup-restore", sub)) => {
            let backup_id = sub.get_one::<String>("backup_id").unwrap();
            let body = serde_json::json!({ "backup_id": backup_id });
            let path = "/admin/backups/restore";
            let t = ctx.log_request("POST", path);
            let result = admin_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("Restore started from backup '{}'", backup_id));
            output::print_item(&result?, ctx.format);
        }

        Some(("backup-delete", sub)) => {
            let backup_id = sub.get_one::<String>("backup_id").unwrap();
            let path = format!("/admin/backups/{}", backup_id);
            let t = ctx.log_request("DELETE", &path);
            let result = admin_delete(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("Backup '{}' deleted", backup_id));
            output::print_item(&result?, ctx.format);
        }

        Some(("configure-ttl", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let ttl_seconds = sub.get_one::<u64>("ttl-seconds").unwrap();
            let strategy = sub.get_one::<String>("strategy");
            let mut body = serde_json::json!({ "ttl_seconds": ttl_seconds });
            if let Some(s) = strategy {
                body.as_object_mut()
                    .unwrap()
                    .insert("strategy".to_string(), Value::String(s.clone()));
            }
            let path = format!("/admin/namespaces/{}/ttl", namespace);
            let t = ctx.log_request("PUT", &path);
            let result = admin_put(&ctx.url, &path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!(
                "TTL configured for '{}': {} seconds",
                namespace, ttl_seconds
            ));
            output::print_item(&result?, ctx.format);
        }

        _ => {
            output::error("Unknown admin subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_admin_command;

    #[test]
    fn admin_cluster_status_subcommand_recognized() {
        build_admin_command()
            .try_get_matches_from(["admin", "cluster-status"])
            .expect("admin cluster-status should parse");
    }

    #[test]
    fn admin_optimize_requires_namespace() {
        assert!(
            build_admin_command()
                .try_get_matches_from(["admin", "optimize"])
                .is_err(),
            "admin optimize without namespace should fail"
        );
    }

    #[test]
    fn admin_backup_restore_requires_backup_id() {
        assert!(
            build_admin_command()
                .try_get_matches_from(["admin", "backup-restore"])
                .is_err(),
            "admin backup-restore without id should fail"
        );
    }

    #[test]
    fn admin_configure_ttl_requires_ttl_seconds() {
        let m = build_admin_command()
            .try_get_matches_from(["admin", "configure-ttl", "my-ns", "--ttl-seconds", "86400"])
            .expect("admin configure-ttl should parse");
        let sub = m.subcommand_matches("configure-ttl").unwrap();
        assert_eq!(*sub.get_one::<u64>("ttl-seconds").unwrap(), 86400u64);
    }
}
