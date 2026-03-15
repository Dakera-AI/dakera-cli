//! Namespace management commands

use anyhow::Result;
use clap::ArgMatches;
use serde::Serialize;
use dakera_client::DakeraClient;

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

            if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
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

        _ => {
            output::error("Unknown namespace subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
