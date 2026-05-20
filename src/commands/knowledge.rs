//! Knowledge graph management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::knowledge::{
    DeduplicateRequest, FullKnowledgeGraphRequest, KnowledgeGraphRequest, SummarizeRequest,
};
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::context::Context;
use crate::output;

#[derive(Debug, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: String,
}

#[derive(Debug, Serialize)]
pub struct EdgeRow {
    pub source: String,
    pub target: String,
    pub similarity: f32,
    pub relationship: String,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroupRow {
    pub group: usize,
    pub memory_ids: String,
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("graph", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory-id").cloned();
            let depth = sub_matches.get_one::<u32>("depth").copied();
            let min_similarity = sub_matches.get_one::<f32>("min-similarity").copied();

            let request = KnowledgeGraphRequest {
                agent_id: agent_id.clone(),
                memory_id,
                depth,
                min_similarity,
            };

            let t = ctx.log_request("POST", &format!("/v1/{}/knowledge/graph", agent_id));
            let response = client.knowledge_graph(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::info(&format!(
                "Knowledge graph: {} nodes, {} edges",
                response.nodes.len(),
                response.edges.len()
            ));

            if !response.nodes.is_empty() {
                let rows: Vec<NodeRow> = response
                    .nodes
                    .into_iter()
                    .map(|n| NodeRow {
                        id: n.id,
                        content: if n.content.len() > 80 {
                            format!("{}...", &n.content[..77])
                        } else {
                            n.content
                        },
                        memory_type: n.memory_type.unwrap_or_else(|| "-".to_string()),
                        importance: n
                            .importance
                            .map(|v| format!("{:.3}", v))
                            .unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }

            if !response.edges.is_empty() {
                println!();
                output::info("Edges:");
                let edge_rows: Vec<EdgeRow> = response
                    .edges
                    .into_iter()
                    .map(|e| EdgeRow {
                        source: e.source,
                        target: e.target,
                        similarity: e.similarity,
                        relationship: e.relationship.unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&edge_rows, ctx.format);
            }

            if let Some(clusters) = response.clusters {
                if !clusters.is_empty() {
                    println!();
                    output::info(&format!("Found {} clusters", clusters.len()));
                    for (i, cluster) in clusters.iter().enumerate() {
                        println!("  Cluster {}: {} nodes", i + 1, cluster.len());
                    }
                }
            }
        }

        Some(("full-graph", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let max_nodes = sub_matches.get_one::<u32>("max-nodes").copied();
            let min_similarity = sub_matches.get_one::<f32>("min-similarity").copied();
            let cluster_threshold = sub_matches.get_one::<f32>("cluster-threshold").copied();
            let max_edges = sub_matches.get_one::<u32>("max-edges").copied();

            let request = FullKnowledgeGraphRequest {
                agent_id: agent_id.clone(),
                max_nodes,
                min_similarity,
                cluster_threshold,
                max_edges_per_node: max_edges,
            };

            let t = ctx.log_request("POST", &format!("/v1/{}/knowledge/full-graph", agent_id));
            let response = client.full_knowledge_graph(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::info(&format!(
                "Full knowledge graph for '{}': {} nodes, {} edges",
                agent_id,
                response.nodes.len(),
                response.edges.len()
            ));

            if !response.nodes.is_empty() {
                let rows: Vec<NodeRow> = response
                    .nodes
                    .into_iter()
                    .map(|n| NodeRow {
                        id: n.id,
                        content: if n.content.len() > 80 {
                            format!("{}...", &n.content[..77])
                        } else {
                            n.content
                        },
                        memory_type: n.memory_type.unwrap_or_else(|| "-".to_string()),
                        importance: n
                            .importance
                            .map(|v| format!("{:.3}", v))
                            .unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }

            if !response.edges.is_empty() {
                println!();
                output::info("Edges:");
                let edge_rows: Vec<EdgeRow> = response
                    .edges
                    .into_iter()
                    .map(|e| EdgeRow {
                        source: e.source,
                        target: e.target,
                        similarity: e.similarity,
                        relationship: e.relationship.unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&edge_rows, ctx.format);
            }

            if let Some(clusters) = response.clusters {
                if !clusters.is_empty() {
                    println!();
                    output::info(&format!("Found {} clusters", clusters.len()));
                    for (i, cluster) in clusters.iter().enumerate() {
                        println!("  Cluster {}: {} nodes", i + 1, cluster.len());
                    }
                }
            }
        }

        Some(("summarize", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_ids = sub_matches
                .get_one::<String>("memory-ids")
                .map(|s| s.split(',').map(|id| id.trim().to_string()).collect());
            let target_type = sub_matches.get_one::<String>("target-type").cloned();
            let dry_run = sub_matches.get_flag("dry-run");

            let request = SummarizeRequest {
                agent_id: agent_id.clone(),
                memory_ids,
                target_type,
                dry_run,
            };

            let t = ctx.log_request("POST", &format!("/v1/{}/knowledge/summarize", agent_id));
            let response = client.summarize(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if dry_run {
                output::info(&format!(
                    "[dry-run] Would summarize {} source memories",
                    response.source_count
                ));
                println!();
                println!("Preview:");
                println!("{}", response.summary);
            } else {
                output::success(&format!("Summarized {} memories", response.source_count));
                if let Some(ref id) = response.new_memory_id {
                    output::info(&format!("New memory ID: {}", id));
                }
                println!();
                println!("{}", response.summary);
            }
        }

        Some(("deduplicate", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let threshold = sub_matches.get_one::<f32>("threshold").copied();
            let memory_type = sub_matches.get_one::<String>("type").cloned();
            let dry_run = sub_matches.get_flag("dry-run");

            let request = DeduplicateRequest {
                agent_id: agent_id.clone(),
                threshold,
                memory_type,
                dry_run,
            };

            let t = ctx.log_request("POST", &format!("/v1/{}/knowledge/deduplicate", agent_id));
            let response = client.deduplicate(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if dry_run {
                output::info(&format!(
                    "[dry-run] Found {} duplicates in {} groups (would remove {})",
                    response.duplicates_found,
                    response.groups.len(),
                    response.removed_count
                ));
            } else {
                output::success(&format!(
                    "Found {} duplicates, removed {}",
                    response.duplicates_found, response.removed_count
                ));
            }

            if !response.groups.is_empty() {
                println!();
                output::info("Duplicate groups:");
                let rows: Vec<DuplicateGroupRow> = response
                    .groups
                    .into_iter()
                    .enumerate()
                    .map(|(i, group)| DuplicateGroupRow {
                        group: i + 1,
                        memory_ids: group.join(", "),
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
