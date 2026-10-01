//! Memory management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::memory::{
    BatchMemoryFilter, BatchRecallRequest, ConsolidateRequest, MemoryType, RecallRequest,
    StoreMemoryRequest, UpdateImportanceRequest,
};
use dakera_client::reqwest::Method;
use dakera_client::{DakeraClient, HybridSearchRequest};
use serde::Serialize;

use crate::api;
use crate::context::Context;
use crate::output;

#[derive(Debug, Serialize)]
pub struct MemoryRow {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: f32,
    pub score: f32,
}

#[derive(Debug, Serialize)]
pub struct HybridRow {
    pub id: String,
    pub score: f32,
}

fn parse_memory_type(s: &str) -> MemoryType {
    match s.to_lowercase().as_str() {
        "semantic" => MemoryType::Semantic,
        "procedural" => MemoryType::Procedural,
        "working" => MemoryType::Working,
        _ => MemoryType::Episodic,
    }
}

fn memory_type_to_string(mt: &MemoryType) -> String {
    match mt {
        MemoryType::Episodic => "episodic".to_string(),
        MemoryType::Semantic => "semantic".to_string(),
        MemoryType::Procedural => "procedural".to_string(),
        MemoryType::Working => "working".to_string(),
    }
}

/// `PUT /v1/memory/update/{id}?agent_id=...` (the SDK 0.11 route does not exist).
fn update_path(agent_id: &str, memory_id: &str) -> String {
    let id = api::segment(memory_id);
    let agent = api::segment(agent_id);
    format!("/v1/memory/update/{id}?agent_id={agent}")
}

/// The `filter` of `DELETE /v1/memories/forget/batch`: type, importance floor
/// and age become `memory_type`, `min_importance` and `created_before`.
fn batch_forget_filter(sub: &ArgMatches) -> serde_json::Value {
    let mut filter = serde_json::json!({});
    if let Some(mt) = sub.get_one::<String>("type") {
        filter["memory_type"] = serde_json::json!(mt);
    }
    if let Some(mi) = sub.get_one::<f32>("min-importance") {
        filter["min_importance"] = serde_json::json!(mi);
    }
    if let Some(days) = sub.get_one::<u32>("max-age-days") {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let cutoff = now.saturating_sub(u64::from(*days) * 86_400);
        filter["created_before"] = serde_json::json!(cutoff);
    }
    filter
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("store", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let content = sub_matches.get_one::<String>("content").unwrap();
            let memory_type = sub_matches
                .get_one::<String>("type")
                .map(|s| parse_memory_type(s))
                .unwrap_or_default();
            let importance = *sub_matches.get_one::<f32>("importance").unwrap();
            let session_id = sub_matches.get_one::<String>("session-id").cloned();

            let mut request = StoreMemoryRequest::new(agent_id.clone(), content.clone())
                .with_type(memory_type)
                .with_importance(importance);

            if let Some(sid) = session_id {
                request = request.with_session(sid);
            }

            let t = ctx.log_request("POST", "/v1/memory/store");
            let response = client.store_memory(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::success(&format!(
                "Memory stored (id: {}, namespace: {})",
                response.memory_id, response.namespace
            ));
        }

        Some(("recall", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let query = sub_matches.get_one::<String>("query").unwrap();
            let top_k = *sub_matches.get_one::<usize>("top-k").unwrap();
            let memory_type = sub_matches.get_one::<String>("type");

            let mut request = RecallRequest::new(agent_id.clone(), query.clone()).with_top_k(top_k);

            if let Some(t) = memory_type {
                request = request.with_type(parse_memory_type(t));
            }

            let t = ctx.log_request("POST", "/v1/memory/recall");
            let response = client.recall(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.memories.is_empty() {
                output::info("No memories found");
            } else {
                output::info(&format!(
                    "Found {} memories (total: {})",
                    response.memories.len(),
                    response.total_found
                ));
                let rows: Vec<MemoryRow> = response
                    .memories
                    .into_iter()
                    .map(|m| MemoryRow {
                        id: m.id,
                        content: m.content,
                        memory_type: memory_type_to_string(&m.memory_type),
                        importance: m.importance,
                        score: m.score,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("get", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory_id").unwrap();

            let t = ctx.log_request("GET", &format!("/v1/memory/get/{}", memory_id));
            let memory = client.get_memory(agent_id, memory_id).await;
            match &memory {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            output::print_item(&memory?, ctx.format);
        }

        Some(("update", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory_id").unwrap();
            let mut body = serde_json::json!({});
            if let Some(content) = sub_matches.get_one::<String>("content") {
                body["content"] = serde_json::json!(content);
            }
            if let Some(t) = sub_matches.get_one::<String>("type") {
                let memory_type = memory_type_to_string(&parse_memory_type(t));
                body["memory_type"] = serde_json::json!(memory_type);
            }
            let path = update_path(agent_id, memory_id);
            let result = api::request_json_logged(ctx, Method::PUT, &path, Some(&body)).await?;
            let id = api::str_of(&result, "id").unwrap_or(memory_id);
            output::success(&format!("Memory '{id}' updated"));
        }

        Some(("forget", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory_id").unwrap();

            let request = dakera_client::memory::ForgetRequest::by_ids(
                agent_id.clone(),
                vec![memory_id.clone()],
            );
            let t = ctx.log_request("POST", "/v1/memory/forget");
            let response = client.forget(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::success(&format!(
                "Deleted {} memory (id: {})",
                response.deleted_count, memory_id
            ));
        }

        Some(("search", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let query = sub_matches.get_one::<String>("query").unwrap();
            let top_k = *sub_matches.get_one::<usize>("top-k").unwrap();
            let memory_type = sub_matches.get_one::<String>("type");

            let mut request = RecallRequest::new(agent_id.clone(), query.clone()).with_top_k(top_k);

            if let Some(t) = memory_type {
                request = request.with_type(parse_memory_type(t));
            }

            let t = ctx.log_request("POST", "/v1/memory/search");
            let response = client.search_memories(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.memories.is_empty() {
                output::info("No memories found");
            } else {
                output::info(&format!(
                    "Found {} memories (total: {})",
                    response.memories.len(),
                    response.total_found
                ));
                let rows: Vec<MemoryRow> = response
                    .memories
                    .into_iter()
                    .map(|m| MemoryRow {
                        id: m.id,
                        content: m.content,
                        memory_type: memory_type_to_string(&m.memory_type),
                        importance: m.importance,
                        score: m.score,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("importance", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let ids: Vec<String> = sub_matches
                .get_one::<String>("ids")
                .unwrap()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let value = *sub_matches.get_one::<f32>("value").unwrap();

            let request = UpdateImportanceRequest {
                memory_ids: ids.clone(),
                importance: value,
            };

            let t = ctx.log_request("POST", "/v1/memory/importance");
            client.update_importance(agent_id, request).await?;
            ctx.log_response(t, "200 OK");
            output::success(&format!(
                "Updated importance to {} for {} memories",
                value,
                ids.len()
            ));
        }

        Some(("consolidate", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_type = sub_matches.get_one::<String>("type").cloned();
            let threshold = sub_matches.get_one::<f32>("threshold").copied();
            let dry_run = sub_matches.get_flag("dry-run");

            let request = ConsolidateRequest {
                memory_type,
                threshold,
                dry_run,
                ..Default::default()
            };

            let t = ctx.log_request("POST", "/v1/memory/consolidate");
            let response = client.consolidate(agent_id, request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if dry_run {
                output::info(&format!(
                    "[dry-run] Would consolidate {} memories, removing {}",
                    response.consolidated_count, response.removed_count
                ));
            } else {
                output::success(&format!(
                    "Consolidated {} memories, removed {} duplicates, created {} new memories",
                    response.consolidated_count,
                    response.removed_count,
                    response.new_memories.len()
                ));
            }
        }

        Some(("feedback", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory_id").unwrap();
            let signal = sub_matches.get_one::<String>("signal").unwrap();
            let body = serde_json::json!({
                "agent_id": agent_id,
                "memory_id": memory_id,
                "signal": signal,
            });
            let path = "/v1/memory/feedback";
            let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            output::success(&format!("Feedback submitted (signal: {signal})"));
            if let Some(importance) = result.get("new_importance").and_then(|v| v.as_f64()) {
                output::info(&format!("New importance: {importance}"));
            }
        }

        Some(("batch-forget", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let dry_run = sub_matches.get_flag("dry-run");
            let filter = batch_forget_filter(sub_matches);
            if dry_run {
                // Count what the filter matches with the batch-recall route.
                let body = serde_json::json!({
                    "agent_id": agent_id,
                    "filter": filter,
                    "limit": 1,
                });
                let path = "/v1/memories/recall/batch";
                let result = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
                let count = api::u64_of(&result, "filtered").unwrap_or(0);
                output::info(&format!("[dry-run] Would delete {count} memories"));
                return Ok(());
            }
            let body = serde_json::json!({ "agent_id": agent_id, "filter": filter });
            let path = "/v1/memories/forget/batch";
            let result = api::request_json_logged(ctx, Method::DELETE, path, Some(&body)).await?;
            let count = api::u64_of(&result, "deleted_count").unwrap_or(0);
            output::success(&format!("Deleted {count} memories"));
        }

        Some(("batch-recall", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let limit = *sub_matches.get_one::<usize>("limit").unwrap();
            let min_importance = sub_matches.get_one::<f32>("min-importance").copied();
            let max_importance = sub_matches.get_one::<f32>("max-importance").copied();
            let memory_type = sub_matches.get_one::<String>("type");
            let session_id = sub_matches.get_one::<String>("session-id").cloned();
            let tags: Option<Vec<String>> = sub_matches
                .get_one::<String>("tags")
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());

            let mut filter = BatchMemoryFilter::default();
            if let Some(t) = tags {
                filter = filter.with_tags(t);
            }
            if let Some(mi) = min_importance {
                filter = filter.with_min_importance(mi);
            }
            if let Some(ma) = max_importance {
                filter = filter.with_max_importance(ma);
            }
            if let Some(mt) = memory_type {
                filter.memory_type = Some(parse_memory_type(mt));
            }
            if let Some(sid) = session_id {
                filter = filter.with_session(sid);
            }

            let request = BatchRecallRequest::new(agent_id.clone())
                .with_filter(filter)
                .with_limit(limit);

            let t = ctx.log_request("POST", "/v1/memories/recall/batch");
            let response = client.batch_recall(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.memories.is_empty() {
                output::info("No memories found");
            } else {
                output::info(&format!(
                    "Found {} memories (total: {}, filtered: {})",
                    response.memories.len(),
                    response.total,
                    response.filtered
                ));
                let rows: Vec<MemoryRow> = response
                    .memories
                    .into_iter()
                    .map(|m| MemoryRow {
                        id: m.id,
                        content: m.content,
                        memory_type: memory_type_to_string(&m.memory_type),
                        importance: m.importance,
                        score: m.score,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("hybrid-search", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let query = sub_matches.get_one::<String>("query").unwrap();
            let top_k = *sub_matches.get_one::<u32>("top-k").unwrap();
            let vector_weight = *sub_matches.get_one::<f32>("vector-weight").unwrap();

            let request = HybridSearchRequest::text_only(query.clone(), top_k)
                .with_vector_weight(vector_weight);

            let t = ctx.log_request("POST", &format!("/v1/namespaces/{}/hybrid", namespace));
            let response = client.hybrid_search(namespace, request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.results.is_empty() {
                output::info("No results found");
            } else {
                output::info(&format!("Found {} results", response.results.len()));
                let rows: Vec<HybridRow> = response
                    .results
                    .into_iter()
                    .map(|m| HybridRow {
                        id: m.id,
                        score: m.score,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown memory subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_memory_type_defaults_to_episodic_for_unknown() {
        assert!(matches!(parse_memory_type("unknown"), MemoryType::Episodic));
        assert!(matches!(parse_memory_type(""), MemoryType::Episodic));
        assert!(matches!(
            parse_memory_type("EPISODIC"),
            MemoryType::Episodic
        ));
    }

    #[test]
    fn parse_memory_type_recognizes_all_variants() {
        assert!(matches!(
            parse_memory_type("episodic"),
            MemoryType::Episodic
        ));
        assert!(matches!(
            parse_memory_type("semantic"),
            MemoryType::Semantic
        ));
        assert!(matches!(
            parse_memory_type("procedural"),
            MemoryType::Procedural
        ));
        assert!(matches!(parse_memory_type("working"), MemoryType::Working));
    }

    #[test]
    fn parse_memory_type_is_case_insensitive() {
        assert!(matches!(
            parse_memory_type("SEMANTIC"),
            MemoryType::Semantic
        ));
        assert!(matches!(
            parse_memory_type("Procedural"),
            MemoryType::Procedural
        ));
        assert!(matches!(parse_memory_type("WORKING"), MemoryType::Working));
    }

    #[test]
    fn memory_type_to_string_returns_lowercase() {
        assert_eq!(memory_type_to_string(&MemoryType::Episodic), "episodic");
        assert_eq!(memory_type_to_string(&MemoryType::Semantic), "semantic");
        assert_eq!(memory_type_to_string(&MemoryType::Procedural), "procedural");
        assert_eq!(memory_type_to_string(&MemoryType::Working), "working");
    }

    #[test]
    fn parse_and_stringify_are_inverses() {
        for s in &["episodic", "semantic", "procedural", "working"] {
            assert_eq!(
                &memory_type_to_string(&parse_memory_type(s)),
                s,
                "round-trip failed for: {s}"
            );
        }
    }

    #[test]
    fn hybrid_row_serializes_id_and_score() {
        let row = HybridRow {
            id: "vec-1".into(),
            score: 0.87,
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["id"], "vec-1");
        assert!((json["score"].as_f64().unwrap() - 0.87).abs() < 1e-6);
    }

    #[test]
    fn memory_row_serializes_all_fields() {
        let row = MemoryRow {
            id: "mem-1".into(),
            content: "hello".into(),
            memory_type: "episodic".into(),
            importance: 0.8,
            score: 0.9,
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["id"], "mem-1");
        assert_eq!(json["memory_type"], "episodic");
    }
}
