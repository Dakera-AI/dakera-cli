//! Namespace management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::output;
use crate::OutputFormat;

#[derive(Debug, Serialize)]
pub struct NamespaceRow {
    pub name: String,
}

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("list", _)) => {
            let namespaces = client.list_namespaces().await?;

            if namespaces.is_empty() {
                output::info("No namespaces found");
            } else {
                let rows: Vec<NamespaceRow> = namespaces
                    .into_iter()
                    .map(|name| NamespaceRow { name })
                    .collect();
                output::print_data(&rows, format);
            }
        }

        Some(("get", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            let info = client.get_namespace(name).await?;
            output::print_item(&info, format);
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

            // Note: Dakera doesn't have a delete namespace endpoint yet
            output::warning("Namespace deletion is not yet implemented in the server");
            output::info("To remove all vectors from a namespace, use 'dk vector delete --all'");
        }

        Some(("policy", sub_matches)) => {
            match sub_matches.subcommand() {
                Some(("get", get_matches)) => {
                    let ns = get_matches.get_one::<String>("namespace").unwrap();
                    let policy = client.get_memory_policy(ns).await?;
                    output::print_item(&policy, format);
                }

                Some(("set", set_matches)) => {
                    let ns = set_matches.get_one::<String>("namespace").unwrap();

                    // Fetch the current policy so we only change what the user specified.
                    let mut policy = client.get_memory_policy(ns).await?;

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

                    let updated = client.set_memory_policy(ns, policy).await?;
                    output::success(&format!("Memory policy updated for namespace '{}'", ns));
                    output::print_item(&updated, format);
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
