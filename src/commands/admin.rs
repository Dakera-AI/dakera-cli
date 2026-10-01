//! Admin commands for cluster management, caching, backups, and configuration

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde_json::Value;

use super::admin_v012;
use crate::context::Context as Ctx;
use crate::output;

async fn admin_get(url: &str, path: &str) -> Result<Value> {
    crate::api::request_json(url, Method::GET, path, None).await
}

async fn admin_post(url: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    crate::api::request_json(url, Method::POST, path, body).await
}

async fn admin_delete(url: &str, path: &str) -> Result<Value> {
    crate::api::request_json(url, Method::DELETE, path, None).await
}

async fn admin_put(url: &str, path: &str, body: &Value) -> Result<Value> {
    crate::api::request_json(url, Method::PUT, path, Some(body)).await
}

/// `PUT /admin/quotas/{namespace}`, or `/admin/quotas/default` without one
/// (the route `PUT /admin/quotas` does not exist).
fn quota_path(namespace: Option<&String>) -> String {
    match namespace {
        Some(ns) => format!("/admin/quotas/{}", crate::api::segment(ns)),
        None => "/admin/quotas/default".to_string(),
    }
}

/// The server wants `{"config": {...}}`; a bare quota config is wrapped.
fn quota_request_body(parsed: Value) -> Value {
    if parsed.get("config").is_some() {
        parsed
    } else {
        serde_json::json!({ "config": parsed })
    }
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("cluster-status", _sub)) => {
            let path = "/admin/cluster/status";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Cluster Status");
            output::print_item(&result, ctx.format);
        }

        Some(("cluster-nodes", _sub)) => {
            let path = "/admin/cluster/nodes";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Cluster Nodes");
            output::print_item(&result, ctx.format);
        }

        Some(("optimize", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = format!("/admin/namespaces/{}/optimize", namespace);
            let t = ctx.log_request("POST", &path);
            let result = admin_post(&ctx.url, &path, None).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::success(&format!("Namespace '{}' optimization started", namespace));
            output::print_item(&result, ctx.format);
        }

        Some(("index-stats", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = format!("/admin/indexes/stats?namespace={}", namespace);
            let t = ctx.log_request("GET", &path);
            let result = admin_get(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info(&format!("Index stats for '{}'", namespace));
            output::print_item(&result, ctx.format);
        }

        Some(("rebuild-indexes", sub)) => {
            let namespace = sub.get_one::<String>("namespace").unwrap();
            let path = "/admin/indexes/rebuild";
            let body = serde_json::json!({ "namespace": namespace });
            let t = ctx.log_request("POST", path);
            let result = admin_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::success(&format!("Index rebuild started for '{}'", namespace));
            output::print_item(&result, ctx.format);
        }

        Some(("cache-stats", _sub)) => {
            let path = "/admin/cache/stats";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Cache Statistics");
            output::print_item(&result, ctx.format);
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
            let result = result?;
            if let Some(ns) = namespace {
                output::success(&format!("Cache cleared for namespace '{}'", ns));
            } else {
                output::success("Cache cleared for all namespaces");
            }
            output::print_item(&result, ctx.format);
        }

        Some(("config-get", _sub)) => {
            let path = "/admin/config";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Server Configuration");
            output::print_item(&result, ctx.format);
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
            let result = result?;
            output::success(&format!("Configuration updated: {} = {}", key, value));
            output::print_item(&result, ctx.format);
        }

        Some(("quotas-get", _sub)) => {
            let path = "/admin/quotas";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Namespace Quotas");
            output::print_item(&result, ctx.format);
        }

        Some(("quotas-set", sub)) => {
            let data = sub.get_one::<String>("data").unwrap();
            let parsed: Value =
                serde_json::from_str(data).with_context(|| "Invalid JSON for --data")?;
            let body = quota_request_body(parsed);
            let path = quota_path(sub.get_one::<String>("namespace"));
            let t = ctx.log_request("PUT", &path);
            let result = admin_put(&ctx.url, &path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::success("Quotas updated");
            output::print_item(&result, ctx.format);
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
            let result = result?;
            output::info("Slow Queries");
            output::print_item(&result, ctx.format);
        }

        Some(("backup-create", sub)) => admin_v012::backup_create(ctx, sub).await?,
        Some(("backup-get", sub)) => admin_v012::backup_get(ctx, sub).await?,
        Some(("backup-download", sub)) => admin_v012::backup_download(ctx, sub).await?,
        Some(("backup-upload", sub)) => admin_v012::backup_upload(ctx, sub).await?,
        Some(("backup-restore-status", sub)) => admin_v012::backup_restore_status(ctx, sub).await?,
        Some(("backup-schedule", sub)) => admin_v012::backup_schedule(ctx, sub).await?,
        Some(("encryption-status", _)) => admin_v012::encryption_status(ctx).await?,
        Some(("encryption-rotate", sub)) => admin_v012::encryption_rotate(ctx, sub).await?,
        Some(("encryption-reseal", sub)) => admin_v012::encryption_reseal(ctx, sub).await?,
        Some(("embed-migration", _)) => admin_v012::embed_migration(ctx).await?,

        Some(("backup-list", _sub)) => {
            let path = "/admin/backups";
            let t = ctx.log_request("GET", path);
            let result = admin_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::info("Backups");
            output::print_item(&result, ctx.format);
        }

        Some(("backup-restore", sub)) => admin_v012::backup_restore(ctx, sub).await?,

        Some(("backup-delete", sub)) => {
            let backup_id = sub.get_one::<String>("backup_id").unwrap();
            let path = format!("/admin/backups/{}", backup_id);
            let t = ctx.log_request("DELETE", &path);
            let result = admin_delete(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;
            output::success(&format!("Backup '{}' deleted", backup_id));
            output::print_item(&result, ctx.format);
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
            let result = result?;
            output::success(&format!(
                "TTL configured for '{}': {} seconds",
                namespace, ttl_seconds
            ));
            output::print_item(&result, ctx.format);
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
    use super::{quota_path, quota_request_body};
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
    fn quota_path_targets_one_namespace_or_the_default() {
        let ns = "team-a".to_string();
        assert_eq!(quota_path(Some(&ns)), "/admin/quotas/team-a");
        assert_eq!(quota_path(None), "/admin/quotas/default");
    }

    #[test]
    fn quota_body_wraps_a_bare_config() {
        let bare = serde_json::json!({ "max_vectors": 10, "enforcement": "hard" });
        let wrapped = quota_request_body(bare.clone());
        assert_eq!(wrapped["config"], bare);
        let full = serde_json::json!({ "config": { "max_vectors": 10 } });
        assert_eq!(quota_request_body(full.clone()), full);
    }

    #[test]
    fn admin_v012_commands_parse() {
        for args in [
            vec!["admin", "embed-migration"],
            vec!["admin", "encryption-status"],
            vec!["admin", "encryption-rotate", "-n", "ns"],
            vec!["admin", "encryption-reseal"],
            vec!["admin", "backup-create", "--wait"],
            vec!["admin", "backup-get", "b1"],
            vec!["admin", "backup-download", "b1", "-o", "out.json.gz"],
            vec!["admin", "backup-upload", "bundle.json.gz"],
            vec!["admin", "backup-restore-status", "r1"],
            vec!["admin", "backup-schedule"],
        ] {
            build_admin_command()
                .try_get_matches_from(args.clone())
                .unwrap_or_else(|e| panic!("{args:?} should parse: {e}"));
        }
    }

    #[test]
    fn admin_backup_download_requires_an_output() {
        assert!(build_admin_command()
            .try_get_matches_from(["admin", "backup-download", "b1"])
            .is_err());
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
