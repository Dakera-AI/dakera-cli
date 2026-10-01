//! Index management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use dakera_client::DakeraClient;

use crate::api;
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
            // Raw REST: the server answers {document_count, unique_terms,
            // avg_doc_length}, which dakera-client 0.12.0 fails to decode.
            let path = format!("/v1/namespaces/{}/fulltext/stats", api::segment(namespace));
            let stats = api::request_json_logged(ctx, Method::GET, &path, None).await?;
            output::print_item(&stats, ctx.format);
        }

        Some(("rebuild", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let index_type = sub_matches.get_one::<String>("index-type");
            let what = index_type.map(String::as_str).unwrap_or("vector");
            let yes = sub_matches.get_flag("yes");
            let dry_run = sub_matches.get_flag("dry-run");

            if dry_run {
                output::info(&format!(
                    "[dry-run] Would rebuild the {} index of namespace '{}' (no action taken)",
                    what, namespace
                ));
                output::info("[dry-run] Re-run without --dry-run to proceed with the rebuild");
                return Ok(());
            }

            if !yes {
                output::warning(&format!(
                    "This will rebuild the {} index of namespace '{}'. This may take some time.",
                    what, namespace
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

            let mut body = serde_json::json!({
                "namespace": namespace,
                "force": sub_matches.get_flag("force"),
            });
            if let Some(t) = index_type {
                body["index_type"] = serde_json::json!(t);
            }
            let result =
                api::request_json_logged(ctx, Method::POST, "/admin/indexes/rebuild", Some(&body))
                    .await?;
            if let Some(msg) = api::str_of(&result, "message") {
                output::success(msg);
            }
            output::print_item(&result, ctx.format);
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
    fn index_rebuild_leaves_the_index_kind_to_the_server() {
        // The server picks flat or HNSW per namespace; without --index-type the
        // request carries none (a different kind is rejected with 400).
        let m = build_index_command()
            .try_get_matches_from(["index", "rebuild", "--namespace", "ns1", "--yes"])
            .expect("index rebuild should parse");
        let sub = m.subcommand_matches("rebuild").unwrap();
        assert!(sub.get_one::<String>("index-type").is_none());
        assert!(!sub.get_flag("force"));
    }

    #[test]
    fn index_rebuild_accepts_force_and_an_expected_kind() {
        let m = build_index_command()
            .try_get_matches_from([
                "index", "rebuild", "-n", "ns1", "-t", "hnsw", "--force", "--yes",
            ])
            .expect("index rebuild --force should parse");
        let sub = m.subcommand_matches("rebuild").unwrap();
        assert_eq!(sub.get_one::<String>("index-type").unwrap(), "hnsw");
        assert!(sub.get_flag("force"));
    }
}
