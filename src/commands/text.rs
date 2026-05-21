//! Full-text (BM25) search commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde::Serialize;
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::output;

#[derive(Debug, Serialize)]
pub struct SearchResultRow {
    pub id: String,
    pub score: String,
    pub content: String,
    pub namespace: String,
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("search", sub)) => {
            let query = sub.get_one::<String>("query").unwrap();
            let namespace = sub.get_one::<String>("namespace").cloned();
            let limit = *sub.get_one::<u32>("limit").unwrap();

            let mut body = serde_json::json!({ "query": query, "limit": limit });
            if let Some(ref ns) = namespace {
                body.as_object_mut()
                    .unwrap()
                    .insert("namespace".to_string(), Value::String(ns.clone()));
            }

            let path = "/v1/fulltext/search";
            let t = ctx.log_request("POST", path);
            let client = super::authed_client();
            let resp = client
                .post(format!("{}{}", ctx.url, path))
                .json(&body)
                .send()
                .await
                .with_context(|| "Failed to POST /v1/fulltext/search")?;
            let status = resp.status();
            let text = resp.text().await?;
            ctx.log_response(t, &status.to_string());
            if !status.is_success() {
                anyhow::bail!("Request failed ({}): {}", status, text);
            }

            let data: Value =
                serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")?;

            let results = data
                .get("results")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();
            let total = data
                .get("total")
                .and_then(|t| t.as_u64())
                .unwrap_or(results.len() as u64);

            output::info(&format!("Found {} result(s)", total));

            if results.is_empty() {
                output::info("No results found");
            } else {
                let rows: Vec<SearchResultRow> = results
                    .iter()
                    .map(|r| SearchResultRow {
                        id: r
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        score: r
                            .get("score")
                            .and_then(|v| v.as_f64())
                            .map(|s| format!("{:.4}", s))
                            .unwrap_or_else(|| "-".to_string()),
                        content: {
                            let c = r
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("-")
                                .to_string();
                            if c.len() > 80 {
                                format!("{}...", &c[..77])
                            } else {
                                c
                            }
                        },
                        namespace: r
                            .get("namespace")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown text subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_text_command;

    #[test]
    fn text_search_requires_query() {
        assert!(
            build_text_command()
                .try_get_matches_from(["text", "search"])
                .is_err(),
            "text search without query should fail"
        );
    }

    #[test]
    fn text_search_with_namespace_flag() {
        let m = build_text_command()
            .try_get_matches_from(["text", "search", "my query", "--namespace", "my-ns"])
            .expect("text search with namespace should parse");
        let sub = m.subcommand_matches("search").unwrap();
        assert_eq!(sub.get_one::<String>("namespace").unwrap(), "my-ns");
    }

    #[test]
    fn text_search_limit_defaults_to_10() {
        let m = build_text_command()
            .try_get_matches_from(["text", "search", "query"])
            .expect("text search should parse");
        let sub = m.subcommand_matches("search").unwrap();
        assert_eq!(*sub.get_one::<u32>("limit").unwrap(), 10u32);
    }
}
