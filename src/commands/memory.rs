//! Memory management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::memory::{
    ConsolidateRequest, FeedbackRequest, MemoryType, RecallRequest, StoreMemoryRequest,
    UpdateImportanceRequest, UpdateMemoryRequest,
};
use dakera_client::DakeraClient;
use serde::Serialize;

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

            let t = ctx.log_request("POST", &format!("/v1/{}/memories", agent_id));
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

            let t = ctx.log_request("POST", &format!("/v1/{}/memories/recall", agent_id));
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

            let t = ctx.log_request("GET", &format!("/v1/memories/{}", memory_id));
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
            let content = sub_matches.get_one::<String>("content").cloned();
            let memory_type = sub_matches
                .get_one::<String>("type")
                .map(|s| parse_memory_type(s));

            let request = UpdateMemoryRequest {
                content,
                metadata: None,
                memory_type,
            };

            let t = ctx.log_request("PUT", &format!("/v1/{}/memories/{}", agent_id, memory_id));
            let response = client.update_memory(agent_id, memory_id, request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            output::success(&format!("Memory '{}' updated", response?.memory_id));
        }

        Some(("forget", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_id = sub_matches.get_one::<String>("memory_id").unwrap();

            let request = dakera_client::memory::ForgetRequest::by_ids(
                agent_id.clone(),
                vec![memory_id.clone()],
            );
            let t = ctx.log_request(
                "DELETE",
                &format!("/v1/{}/memories/{}", agent_id, memory_id),
            );
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

            let t = ctx.log_request("POST", &format!("/v1/{}/memories/search", agent_id));
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

            let t = ctx.log_request("PUT", &format!("/v1/{}/memories/importance", agent_id));
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

            let t = ctx.log_request("POST", &format!("/v1/{}/memories/consolidate", agent_id));
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
            let feedback = sub_matches.get_one::<String>("feedback").unwrap();
            let score = sub_matches.get_one::<f32>("score").copied();

            let request = FeedbackRequest {
                memory_id: memory_id.clone(),
                feedback: feedback.clone(),
                relevance_score: score,
            };

            let t = ctx.log_request("POST", &format!("/v1/{}/memories/feedback", agent_id));
            let response = client.memory_feedback(agent_id, request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::success(&format!("Feedback submitted (status: {})", response.status));
            if let Some(importance) = response.updated_importance {
                output::info(&format!("Updated importance: {}", importance));
            }
        }

        Some(("batch-forget", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_type = sub_matches.get_one::<String>("type").cloned();
            let min_importance = sub_matches.get_one::<f32>("min-importance").copied();
            let max_age_days = sub_matches.get_one::<u32>("max-age-days").copied();
            let dry_run = sub_matches.get_flag("dry-run");

            let mut filters = serde_json::json!({});
            if let Some(ref mt) = memory_type {
                filters["memory_type"] = serde_json::Value::String(mt.clone());
            }
            if let Some(mi) = min_importance {
                filters["min_importance"] = serde_json::json!(mi);
            }
            if let Some(age) = max_age_days {
                filters["max_age_days"] = serde_json::json!(age);
            }
            if dry_run {
                filters["dry_run"] = serde_json::Value::Bool(true);
            }

            let body = serde_json::json!({
                "agent_id": agent_id,
                "filters": filters
            });

            let path = "/v1/memories/forget/batch";
            let t = ctx.log_request("POST", path);
            let http_client = super::authed_client();
            let resp = http_client
                .post(format!("{}{}", ctx.url, path))
                .json(&body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to POST {}: {}", path, e))?;
            let status = resp.status();
            let text = resp.text().await?;
            ctx.log_response(t, &status.to_string());
            if !status.is_success() {
                anyhow::bail!("Request failed ({}): {}", status, text);
            }

            let data: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::json!({ "deleted_count": 0 }));
            let count = data
                .get("deleted_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if dry_run {
                output::info(&format!("[dry-run] Would delete {} memories", count));
            } else {
                output::success(&format!("Deleted {} memories", count));
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
}
