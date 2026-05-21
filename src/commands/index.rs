//! Index management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;

use crate::context::Context;
use crate::output;

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("stats", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let t = ctx.log_request("GET", &format!("/v1/namespaces/{}", namespace));
            let info = client.get_namespace(namespace).await;
            match &info {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let info = info?;

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
                ctx.format,
            );
        }

        Some(("fulltext-stats", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let t = ctx.log_request("GET", &format!("/v1/{}/fulltext/stats", namespace));
            let stats = client.fulltext_stats(namespace).await;
            match &stats {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            output::print_item(&stats?, ctx.format);
        }

        Some(("rebuild", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let index_type = sub_matches.get_one::<String>("index-type").unwrap();
            let yes = sub_matches.get_flag("yes");
            let dry_run = sub_matches.get_flag("dry-run");

            if dry_run {
                output::info(&format!(
                    "[dry-run] Would rebuild {} index for namespace '{}' (no action taken)",
                    index_type, namespace
                ));
                output::info("[dry-run] Re-run without --dry-run to proceed with the rebuild");
                return Ok(());
            }

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

#[cfg(test)]
mod tests {
    use crate::cli::build_index_command;

    #[test]
    fn index_stats_requires_namespace() {
        assert!(
            build_index_command()
                .try_get_matches_from(["index", "stats"])
                .is_err(),
            "index stats without --namespace should fail"
        );
    }

    #[test]
    fn index_rebuild_dry_run_flag_works() {
        let m = build_index_command()
            .try_get_matches_from(["index", "rebuild", "--namespace", "ns1", "--dry-run"])
            .expect("index rebuild --dry-run should parse");
        let sub = m.subcommand_matches("rebuild").unwrap();
        assert!(sub.get_flag("dry-run"));
    }

    #[test]
    fn index_rebuild_index_type_defaults_to_all() {
        let m = build_index_command()
            .try_get_matches_from(["index", "rebuild", "--namespace", "ns1", "--yes"])
            .expect("index rebuild should parse");
        let sub = m.subcommand_matches("rebuild").unwrap();
        assert_eq!(sub.get_one::<String>("index-type").unwrap(), "all");
    }
}
