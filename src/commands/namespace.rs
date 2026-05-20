//! Namespace management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::context::Context;
use crate::output;

#[derive(Debug, Serialize)]
pub struct NamespaceRow {
    pub name: String,
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("list", _)) => {
            let t = ctx.log_request("GET", "/v1/namespaces");
            let namespaces = client.list_namespaces().await;
            match &namespaces {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let namespaces = namespaces?;

            if namespaces.is_empty() {
                output::info("No namespaces found");
            } else {
                let rows: Vec<NamespaceRow> = namespaces
                    .into_iter()
                    .map(|name| NamespaceRow { name })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("get", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            let path = format!("/v1/namespaces/{}", name);
            let t = ctx.log_request("GET", &path);
            let info = client.get_namespace(name).await;
            match &info {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            output::print_item(&info?, ctx.format);
        }

        Some(("create", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();

            if name.is_empty() {
                output::error("Namespace name cannot be empty");
                std::process::exit(1);
            }

            if !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                output::error("Namespace name can only contain alphanumeric characters, hyphens, and underscores");
                std::process::exit(1);
            }

            output::success(&format!(
                "Namespace '{}' will be created on first vector upsert",
                name
            ));
            output::info("Use 'dk vector upsert' to add vectors and create the namespace");
        }

        Some(("delete", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            let yes = sub_matches.get_flag("yes");
            let dry_run = sub_matches.get_flag("dry-run");

            if dry_run {
                output::info(&format!(
                    "[dry-run] Would delete namespace '{}' and all its vectors (no action taken)",
                    name
                ));
                output::info("[dry-run] Re-run without --dry-run to proceed with deletion");
                return Ok(());
            }

            if !yes {
                output::warning(&format!(
                    "This will permanently delete namespace '{}' and all its data",
                    name
                ));
                print!("Are you sure? [y/N]: ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    output::info("Deletion cancelled");
                    return Ok(());
                }
            }

            output::warning("Namespace deletion is not yet implemented in the server");
            output::info("To remove all vectors from a namespace, use 'dk vector delete --all'");
        }

        Some(("policy", sub_matches)) => {
            match sub_matches.subcommand() {
                Some(("get", get_matches)) => {
                    let ns = get_matches.get_one::<String>("namespace").unwrap();
                    let path = format!("/v1/namespaces/{}/memory_policy", ns);
                    let t = ctx.log_request("GET", &path);
                    let policy = client.get_memory_policy(ns).await;
                    match &policy {
                        Ok(_) => ctx.log_response(t, "200 OK"),
                        Err(_) => ctx.log_response(t, "ERR"),
                    }
                    output::print_item(&policy?, ctx.format);
                }

                Some(("set", set_matches)) => {
                    let ns = set_matches.get_one::<String>("namespace").unwrap();

                    // Fetch the current policy so we only change what the user specified.
                    let t = ctx.log_request("GET", &format!("/v1/namespaces/{}/memory_policy", ns));
                    let policy = client.get_memory_policy(ns).await;
                    match &policy {
                        Ok(_) => ctx.log_response(t, "200 OK"),
                        Err(_) => ctx.log_response(t, "ERR"),
                    }
                    let mut policy = policy?;

                    if let Some(v) = set_matches.get_one::<u64>("working-ttl") {
                        policy.working_ttl_seconds = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u64>("episodic-ttl") {
                        policy.episodic_ttl_seconds = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u64>("semantic-ttl") {
                        policy.semantic_ttl_seconds = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u64>("procedural-ttl") {
                        policy.procedural_ttl_seconds = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<String>("working-decay") {
                        policy.working_decay = Some(v.clone());
                    }
                    if let Some(v) = set_matches.get_one::<String>("episodic-decay") {
                        policy.episodic_decay = Some(v.clone());
                    }
                    if let Some(v) = set_matches.get_one::<String>("semantic-decay") {
                        policy.semantic_decay = Some(v.clone());
                    }
                    if let Some(v) = set_matches.get_one::<String>("procedural-decay") {
                        policy.procedural_decay = Some(v.clone());
                    }
                    if let Some(v) = set_matches.get_one::<f64>("spaced-repetition-factor") {
                        policy.spaced_repetition_factor = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u64>("spaced-repetition-base-interval") {
                        policy.spaced_repetition_base_interval_seconds = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<bool>("consolidation-enabled") {
                        policy.consolidation_enabled = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<f32>("consolidation-threshold") {
                        policy.consolidation_threshold = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u32>("consolidation-interval-hours") {
                        policy.consolidation_interval_hours = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<bool>("rate-limit-enabled") {
                        policy.rate_limit_enabled = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u32>("rate-limit-stores-per-minute") {
                        policy.rate_limit_stores_per_minute = Some(*v);
                    }
                    if let Some(v) = set_matches.get_one::<u32>("rate-limit-recalls-per-minute") {
                        policy.rate_limit_recalls_per_minute = Some(*v);
                    }

                    // consolidated_count is read-only — clear it before sending
                    policy.consolidated_count = None;

                    let path = format!("/v1/namespaces/{}/memory_policy", ns);
                    let t = ctx.log_request("PUT", &path);
                    let updated = client.set_memory_policy(ns, policy).await;
                    match &updated {
                        Ok(_) => ctx.log_response(t, "200 OK"),
                        Err(_) => ctx.log_response(t, "ERR"),
                    }
                    output::success(&format!("Memory policy updated for namespace '{}'", ns));
                    output::print_item(&updated?, ctx.format);
                }

                _ => {
                    output::error("Unknown policy subcommand. Use 'get' or 'set'.");
                    std::process::exit(1);
                }
            }
        }

        _ => {
            output::error("Unknown namespace subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_namespace_command;

    #[test]
    fn namespace_list_subcommand_recognized() {
        build_namespace_command()
            .try_get_matches_from(["namespace", "list"])
            .expect("namespace list should parse");
    }

    #[test]
    fn namespace_delete_requires_name() {
        assert!(
            build_namespace_command()
                .try_get_matches_from(["namespace", "delete"])
                .is_err(),
            "namespace delete without name should fail"
        );
    }

    #[test]
    fn namespace_delete_dry_run_flag_works() {
        let m = build_namespace_command()
            .try_get_matches_from(["namespace", "delete", "my-ns", "--dry-run"])
            .expect("namespace delete --dry-run should parse");
        let sub = m.subcommand_matches("delete").unwrap();
        assert!(sub.get_flag("dry-run"));
    }

    #[test]
    fn namespace_policy_set_rate_limit_flag_parsed() {
        let m = build_namespace_command()
            .try_get_matches_from([
                "namespace",
                "policy",
                "set",
                "my-ns",
                "--rate-limit-enabled",
                "true",
                "--rate-limit-stores-per-minute",
                "60",
            ])
            .expect("namespace policy set should parse");
        let policy = m.subcommand_matches("policy").unwrap();
        let set = policy.subcommand_matches("set").unwrap();
        assert!(*set.get_one::<bool>("rate-limit-enabled").unwrap());
        assert_eq!(
            *set.get_one::<u32>("rate-limit-stores-per-minute").unwrap(),
            60u32
        );
    }
}
