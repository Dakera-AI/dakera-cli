//! Index management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;

use crate::output;
use crate::OutputFormat;

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("stats", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let info = client.get_namespace(namespace).await?;

            let pairs = [
                ("Namespace", namespace.clone()),
                ("Vector Count", info.vector_count.to_string()),
                (
                    "Dimension",
                    info.dimensions
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "Not set".to_string()),
                ),
                (
                    "Index Type",
                    info.index_type
                        .clone()
                        .unwrap_or_else(|| "Auto".to_string()),
                ),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                format,
            );
        }

        Some(("fulltext-stats", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let stats = client.fulltext_stats(namespace).await?;
            output::print_item(&stats, format);
        }

        Some(("rebuild", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let index_type = sub_matches.get_one::<String>("index-type").unwrap();
            let yes = sub_matches.get_flag("yes");

            if !yes {
                output::warning(&format!(
                    "This will rebuild the {} index for namespace '{}'. This may take some time.",
                    index_type, namespace
                ));
                print!("Continue? [y/N]: ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    output::info("Rebuild cancelled");
                    return Ok(());
                }
            }

            output::info(&format!(
                "Triggering {} index rebuild for '{}'...",
                index_type, namespace
            ));

            // Note: This would call a rebuild endpoint when available
            // For now, compaction can help optimize indexes
            output::warning("Direct index rebuild not yet available");
            output::info("Use 'dk ops compact' to optimize storage and indexes");
        }

        _ => {
            output::error("Unknown index subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
