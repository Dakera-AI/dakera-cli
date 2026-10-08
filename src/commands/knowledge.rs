//! Knowledge graph management commands
//!
//! These call the server's REST API directly (like `dk admin`): the request and
//! answer shapes below are the v0.12.0 server's (`crates/common/src/types`),
//! which `dakera-client` 0.12.0 does not match for these routes.

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};

use crate::api;
use crate::context::Context;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

#[derive(Debug, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: String,
}

#[derive(Debug, Serialize)]
pub struct RelatedRow {
    pub memory_id: String,
    pub similarity: String,
    pub shared_tags: String,
}

#[derive(Debug, Serialize)]
pub struct FullNodeRow {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: String,
    pub cluster: u64,
    pub centrality: String,
}

#[derive(Debug, Serialize)]
pub struct EdgeRow {
    pub source: String,
    pub target: String,
    pub similarity: String,
    pub shared_tags: String,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroupRow {
    pub canonical_id: String,
    pub duplicate_ids: String,
    pub avg_similarity: String,
}

fn preview(content: &str) -> String {
    if content.chars().count() > 80 {
        let cut: String = content.chars().take(77).collect();
        format!("{cut}...")
    } else {
        content.to_string()
    }
}

fn f3(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_f64)
        .map(|x| format!("{x:.3}"))
        .unwrap_or_else(|| "-".to_string())
}

fn tags(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn s(v: &Value, key: &str) -> String {
    api::str_of(v, key).unwrap_or("-").to_string()
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// `POST /v1/knowledge/summarize` body: the server needs at least two ids and
/// always stores the summary (it has no dry run).
fn summarize_body(agent_id: &str, ids: &str, target_type: Option<&String>) -> Result<Value> {
    let memory_ids: Vec<&str> = ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect();
    if memory_ids.len() < 2 {
        return Err(input_error(
            "--memory-ids needs at least two memory ids (the server summarizes a group)",
        ));
    }
    let mut body = json!({ "agent_id": agent_id, "memory_ids": memory_ids });
    if let Some(t) = target_type {
        body["target_type"] = json!(t);
    }
    Ok(body)
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("graph", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory-id").unwrap();
            let mut body = json!({ "agent_id": agent_id, "memory_id": memory_id });
            if let Some(depth) = sub_matches.get_one::<u32>("depth") {
                body["depth"] = json!(depth);
            }
            if let Some(ms) = sub_matches.get_one::<f32>("min-similarity") {
                body["min_similarity"] = json!(ms);
            }
            let path = "/v1/knowledge/graph";
            let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            if matches!(ctx.format, OutputFormat::Json) {
                output::print_item(&result, ctx.format);
                return Ok(());
            }

            let root = result.get("root").cloned().unwrap_or(Value::Null);
            let memory = root.get("memory").cloned().unwrap_or(Value::Null);
            output::info(&format!(
                "Knowledge graph from {}: {} nodes",
                s(&memory, "id"),
                api::u64_of(&result, "total_nodes").unwrap_or(0)
            ));
            let root_row = NodeRow {
                id: s(&memory, "id"),
                content: preview(api::str_of(&memory, "content").unwrap_or("")),
                memory_type: s(&memory, "memory_type"),
                importance: f3(&memory, "importance"),
            };
            output::print_data(&[root_row], ctx.format);
            let related: Vec<RelatedRow> = array(&root, "related")
                .iter()
                .map(|r| RelatedRow {
                    memory_id: s(r, "memory_id"),
                    similarity: f3(r, "similarity"),
                    shared_tags: tags(r, "shared_tags"),
                })
                .collect();
            if !related.is_empty() {
                println!();
                output::info("Related memories:");
                output::print_data(&related, ctx.format);
            }
        }

        Some(("full-graph", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let mut body = json!({ "agent_id": agent_id });
            if let Some(v) = sub_matches.get_one::<u32>("max-nodes") {
                body["max_nodes"] = json!(v);
            }
            if let Some(v) = sub_matches.get_one::<f32>("min-similarity") {
                body["min_similarity"] = json!(v);
            }
            if let Some(v) = sub_matches.get_one::<f32>("cluster-threshold") {
                body["cluster_threshold"] = json!(v);
            }
            if let Some(v) = sub_matches.get_one::<u32>("max-edges") {
                body["max_edges_per_node"] = json!(v);
            }
            if let Some(v) = sub_matches.get_one::<u32>("preview") {
                body["content_preview_chars"] = json!(v);
            }
            let path = "/v1/knowledge/graph/full";
            let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            if matches!(ctx.format, OutputFormat::Json) {
                output::print_item(&result, ctx.format);
                return Ok(());
            }

            let nodes = array(&result, "nodes");
            let edges = array(&result, "edges");
            output::info(&format!(
                "Full knowledge graph for '{agent_id}': {} nodes, {} edges",
                nodes.len(),
                edges.len()
            ));
            if !nodes.is_empty() {
                let rows: Vec<FullNodeRow> = nodes
                    .iter()
                    .map(|n| FullNodeRow {
                        id: s(n, "id"),
                        content: preview(api::str_of(n, "content").unwrap_or("")),
                        memory_type: s(n, "memory_type"),
                        importance: f3(n, "importance"),
                        cluster: api::u64_of(n, "cluster_id").unwrap_or(0),
                        centrality: f3(n, "centrality"),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
            if !edges.is_empty() {
                println!();
                output::info("Edges:");
                let rows: Vec<EdgeRow> = edges
                    .iter()
                    .map(|e| EdgeRow {
                        source: s(e, "source"),
                        target: s(e, "target"),
                        similarity: f3(e, "similarity"),
                        shared_tags: tags(e, "shared_tags"),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
            let clusters = array(&result, "clusters");
            if !clusters.is_empty() {
                println!();
                output::info(&format!("Found {} clusters", clusters.len()));
                for c in clusters {
                    println!(
                        "  Cluster {}: {} nodes, avg importance {}, top tags: {}",
                        api::u64_of(c, "id").unwrap_or(0),
                        api::u64_of(c, "node_count").unwrap_or(0),
                        f3(c, "avg_importance"),
                        tags(c, "top_tags")
                    );
                }
            }
            if let Some(hub) = api::ptr_str(&result, "/stats/hub_memory_id") {
                output::info(&format!("Hub memory: {hub}"));
            }
        }

        Some(("summarize", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let ids = sub_matches.get_one::<String>("memory-ids").unwrap();
            let body = summarize_body(agent_id, ids, sub_matches.get_one::<String>("target-type"))?;
            let path = "/v1/knowledge/summarize";
            let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            if matches!(ctx.format, OutputFormat::Json) {
                output::print_item(&result, ctx.format);
                return Ok(());
            }
            let summary = result.get("summary_memory").cloned().unwrap_or(Value::Null);
            output::success(&format!(
                "Summarized {} memories into {}",
                api::u64_of(&result, "source_count").unwrap_or(0),
                s(&summary, "id")
            ));
            println!();
            println!("{}", api::str_of(&summary, "content").unwrap_or(""));
        }

        Some(("deduplicate", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let dry_run = sub_matches.get_flag("dry-run");
            let mut body = json!({ "agent_id": agent_id, "dry_run": dry_run });
            if let Some(t) = sub_matches.get_one::<f32>("threshold") {
                body["threshold"] = json!(t);
            }
            if let Some(mt) = sub_matches.get_one::<String>("type") {
                body["memory_type"] = json!(mt);
            }
            let path = "/v1/knowledge/deduplicate";
            let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            if matches!(ctx.format, OutputFormat::Json) {
                output::print_item(&result, ctx.format);
                return Ok(());
            }
            let groups = array(&result, "groups");
            let found = api::u64_of(&result, "duplicates_found").unwrap_or(0);
            if dry_run {
                output::info(&format!(
                    "[dry-run] Found {found} duplicates in {} groups (nothing merged)",
                    groups.len()
                ));
            } else {
                output::success(&format!(
                    "Found {found} duplicates, merged {}",
                    api::u64_of(&result, "duplicates_merged").unwrap_or(0)
                ));
            }
            if !groups.is_empty() {
                println!();
                output::info("Duplicate groups:");
                let rows: Vec<DuplicateGroupRow> = groups
                    .iter()
                    .map(|g| DuplicateGroupRow {
                        canonical_id: s(g, "canonical_id"),
                        duplicate_ids: tags(g, "duplicate_ids"),
                        avg_similarity: f3(g, "avg_similarity"),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown knowledge subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_knowledge_command;

    #[test]
    fn knowledge_graph_requires_agent_id() {
        assert!(
            build_knowledge_command()
                .try_get_matches_from(["knowledge", "graph"])
                .is_err(),
            "knowledge graph without agent_id should fail"
        );
    }

    #[test]
    fn knowledge_graph_with_depth_flag() {
        let m = build_knowledge_command()
            .try_get_matches_from([
                "knowledge",
                "graph",
                "test-agent",
                "--memory-id",
                "m1",
                "--depth",
                "3",
            ])
            .expect("knowledge graph with --depth should parse");
        let sub = m.subcommand_matches("graph").unwrap();
        assert_eq!(*sub.get_one::<u32>("depth").unwrap(), 3u32);
    }

    #[test]
    fn knowledge_graph_requires_a_seed_memory() {
        // The server answers 422 without `memory_id`.
        assert!(build_knowledge_command()
            .try_get_matches_from(["knowledge", "graph", "test-agent"])
            .is_err());
    }

    #[test]
    fn knowledge_summarize_requires_ids_and_has_no_dry_run() {
        assert!(build_knowledge_command()
            .try_get_matches_from(["knowledge", "summarize", "test-agent"])
            .is_err());
        // The server always stores the summary; a dry run would be a lie.
        assert!(build_knowledge_command()
            .try_get_matches_from([
                "knowledge",
                "summarize",
                "test-agent",
                "--memory-ids",
                "m1,m2",
                "--dry-run",
            ])
            .is_err());
    }

    #[test]
    fn summarize_body_needs_two_ids() {
        assert!(super::summarize_body("a", "m1", None).is_err());
        assert!(super::summarize_body("a", "m1, ,", None).is_err());
        let t = "semantic".to_string();
        let body = super::summarize_body("a", "m1, m2", Some(&t)).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"agent_id": "a", "memory_ids": ["m1", "m2"], "target_type": "semantic"})
        );
    }

    #[test]
    fn knowledge_deduplicate_dry_run_flag_works() {
        let m = build_knowledge_command()
            .try_get_matches_from(["knowledge", "deduplicate", "test-agent", "--dry-run"])
            .expect("knowledge deduplicate --dry-run should parse");
        let sub = m.subcommand_matches("deduplicate").unwrap();
        assert!(sub.get_flag("dry-run"));
    }

    #[test]
    fn knowledge_summarize_accepts_memory_ids() {
        let m = build_knowledge_command()
            .try_get_matches_from([
                "knowledge",
                "summarize",
                "test-agent",
                "--memory-ids",
                "m1,m2,m3",
            ])
            .expect("knowledge summarize with --memory-ids should parse");
        let sub = m.subcommand_matches("summarize").unwrap();
        assert_eq!(sub.get_one::<String>("memory-ids").unwrap(), "m1,m2,m3");
    }
}
