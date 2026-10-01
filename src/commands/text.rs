//! Full-text (BM25) search commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::error::input_error;
use crate::output;

#[derive(Debug, Serialize)]
pub struct SearchResultRow {
    pub id: String,
    pub score: String,
    pub content: String,
    pub namespace: String,
}

/// A result's text: the server returns `id`, `score` and `metadata`; the
/// memory text is `metadata.content` when the index stored it.
fn result_row(r: &Value, namespace: &str) -> SearchResultRow {
    let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("-");
    let score = match r.get("score").and_then(|v| v.as_f64()) {
        Some(s) => format!("{s:.4}"),
        None => "-".to_string(),
    };
    let content = r.pointer("/metadata/content").and_then(|v| v.as_str());
    let content = content.unwrap_or("-");
    let content = if content.chars().count() > 80 {
        let head: String = content.chars().take(77).collect();
        format!("{head}...")
    } else {
        content.to_string()
    };
    SearchResultRow {
        id: id.to_string(),
        score,
        content,
        namespace: namespace.to_string(),
    }
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("search", sub)) => {
            let query = sub.get_one::<String>("query").unwrap();
            let limit = *sub.get_one::<u32>("limit").unwrap();
            // The server searches one namespace: POST /v1/namespaces/{ns}/fulltext/search.
            let Some(namespace) = sub.get_one::<String>("namespace") else {
                return Err(input_error(
                    "text search needs --namespace: the server searches one namespace at a time",
                ));
            };
            let body = serde_json::json!({ "query": query, "top_k": limit });
            let ns = crate::api::segment(namespace);
            let path = format!("/v1/namespaces/{ns}/fulltext/search");
            let data =
                crate::api::request_json_logged(ctx, Method::POST, &path, Some(&body)).await?;
            let results = data
                .get("results")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();

            output::info(&format!("Found {} result(s)", results.len()));
            if results.is_empty() {
                output::info("No results found");
            } else {
                let rows: Vec<SearchResultRow> =
                    results.iter().map(|r| result_row(r, namespace)).collect();
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
