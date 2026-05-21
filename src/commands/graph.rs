//! Graph traversal and export commands (distinct from knowledge graph)

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde::Serialize;
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::output;

#[derive(Debug, Serialize)]
pub struct PathNodeRow {
    pub step: usize,
    pub memory_id: String,
    pub content: String,
    pub relationship: String,
}

#[derive(Debug, Serialize)]
pub struct TraverseNodeRow {
    pub id: String,
    pub content: String,
    pub depth: String,
}

async fn graph_post(url: &str, path: &str, body: &Value) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}{}", url, path))
        .json(body)
        .send()
        .await
        .with_context(|| format!("Failed to POST {}", path))?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("export", sub)) => {
            let agent_id = sub.get_one::<String>("agent_id").unwrap();
            let format = sub.get_one::<String>("fmt").cloned();

            let mut body = serde_json::json!({ "agent_id": agent_id });
            if let Some(ref fmt) = format {
                body.as_object_mut()
                    .unwrap()
                    .insert("format".to_string(), Value::String(fmt.clone()));
            }

            let path = "/v1/graph/export";
            let t = ctx.log_request("POST", path);
            let result = graph_post(&ctx.url, path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

            output::success(&format!("Graph exported for agent '{}'", agent_id));
            output::print_item(&result, ctx.format);
        }

        Some(("path", sub)) => {
            let agent_id = sub.get_one::<String>("agent_id").unwrap();
            let from_id = sub.get_one::<String>("from_id").unwrap();
            let to_id = sub.get_one::<String>("to_id").unwrap();
            let max_depth = sub.get_one::<u32>("max-depth").copied();

            let mut body = serde_json::json!({
                "agent_id": agent_id,
                "from_id": from_id,
                "to_id": to_id
            });
            if let Some(d) = max_depth {
                body.as_object_mut()
                    .unwrap()
                    .insert("max_depth".to_string(), Value::Number(d.into()));
            }

            let path = "/v1/graph/path";
            let t = ctx.log_request("POST", path);
            let result = graph_post(&ctx.url, path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

            let length = result.get("length").and_then(|v| v.as_u64()).unwrap_or(0);
            output::info(&format!(
                "Path from '{}' to '{}': {} hops",
                from_id, to_id, length
            ));

            if let Some(nodes) = result.get("path").and_then(|p| p.as_array()) {
                let rows: Vec<PathNodeRow> = nodes
                    .iter()
                    .enumerate()
                    .map(|(i, n)| PathNodeRow {
                        step: i + 1,
                        memory_id: n
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        content: {
                            let c = n
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("-")
                                .to_string();
                            if c.len() > 60 {
                                format!("{}...", &c[..57])
                            } else {
                                c
                            }
                        },
                        relationship: n
                            .get("relationship")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("traverse", sub)) => {
            let agent_id = sub.get_one::<String>("agent_id").unwrap();
            let start_id = sub.get_one::<String>("start_id").unwrap();
            let depth = sub.get_one::<u32>("depth").copied();
            let max_nodes = sub.get_one::<u32>("max-nodes").copied();

            let mut body = serde_json::json!({
                "agent_id": agent_id,
                "start_id": start_id
            });
            if let Some(d) = depth {
                body.as_object_mut()
                    .unwrap()
                    .insert("depth".to_string(), Value::Number(d.into()));
            }
            if let Some(n) = max_nodes {
                body.as_object_mut()
                    .unwrap()
                    .insert("max_nodes".to_string(), Value::Number(n.into()));
            }

            let path = "/v1/graph/traverse";
            let t = ctx.log_request("POST", path);
            let result = graph_post(&ctx.url, path, &body).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

            let nodes = result
                .get("nodes")
                .and_then(|n| n.as_array())
                .cloned()
                .unwrap_or_default();
            let edges = result
                .get("edges")
                .and_then(|e| e.as_array())
                .cloned()
                .unwrap_or_default();

            output::info(&format!(
                "Traversal from '{}': {} nodes, {} edges",
                start_id,
                nodes.len(),
                edges.len()
            ));

            if !nodes.is_empty() {
                let rows: Vec<TraverseNodeRow> = nodes
                    .iter()
                    .map(|n| TraverseNodeRow {
                        id: n
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        content: {
                            let c = n
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("-")
                                .to_string();
                            if c.len() > 70 {
                                format!("{}...", &c[..67])
                            } else {
                                c
                            }
                        },
                        depth: n
                            .get("depth")
                            .and_then(|v| v.as_u64())
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown graph subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_graph_command;

    #[test]
    fn graph_export_requires_agent_id() {
        assert!(
            build_graph_command()
                .try_get_matches_from(["graph", "export"])
                .is_err(),
            "graph export without agent_id should fail"
        );
    }

    #[test]
    fn graph_path_requires_from_and_to() {
        assert!(
            build_graph_command()
                .try_get_matches_from(["graph", "path", "agent1", "from-id"])
                .is_err(),
            "graph path without to_id should fail"
        );
    }

    #[test]
    fn graph_traverse_requires_start_id() {
        assert!(
            build_graph_command()
                .try_get_matches_from(["graph", "traverse", "agent1"])
                .is_err(),
            "graph traverse without start_id should fail"
        );
    }

    #[test]
    fn graph_traverse_depth_defaults_to_3() {
        let m = build_graph_command()
            .try_get_matches_from(["graph", "traverse", "agent1", "start-mem"])
            .expect("graph traverse should parse");
        let sub = m.subcommand_matches("traverse").unwrap();
        assert_eq!(*sub.get_one::<u32>("depth").unwrap(), 3u32);
    }
}
