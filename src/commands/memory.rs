//! Memory management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::memory::{
    BatchMemoryFilter, BatchRecallRequest, BatchStoreMemoryItem, BatchStoreMemoryRequest,
    ConsolidateRequest, MemoryType, RecallRequest, StoreMemoryRequest, UpdateImportanceRequest,
};
use dakera_client::reqwest::Method;
use dakera_client::{DakeraClient, HybridSearchRequest};
use serde::Serialize;

use crate::api;
use crate::context::Context;
use crate::error::input_error;
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
pub struct StoredRow {
    pub id: String,
    pub content: String,
    pub importance: f32,
}

#[derive(Debug, Serialize)]
pub struct EntityRow {
    pub entity_type: String,
    pub value: String,
    pub score: f64,
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

/// "Found N memories", with the server's total only when it reports a larger
/// one (`POST /v1/memory/recall` reports no total; `search` reports
/// `total_count`).
fn found_message(returned: usize, total: usize) -> String {
    if total > returned {
        format!("Found {returned} memories (total: {total})")
    } else {
        format!("Found {returned} memories")
    }
}

/// One ranked result of `dk memory recall`.
#[derive(Debug, Serialize)]
pub struct RecallRow {
    /// Memory id.
    pub id: String,
    /// Memory content (stored data, shown as is).
    pub content: String,
    /// `episodic`, `semantic`, `procedural` or `working`.
    pub memory_type: String,
    /// Importance, 0.0 to 1.0.
    pub importance: f32,
    /// The ranking score (`smart_score`, else `weighted_score`, else `score`).
    pub score: f32,
    /// The derived sentence that matched best, when the server collapsed
    /// sentence sub-memories into this source memory and the sentence
    /// outranked the source's own text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_text: Option<String>,
    /// The ids of the collapsed sentence sub-memories, best first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_ids: Option<Vec<String>>,
    /// Where the result comes from, when it is a chunk of an imported
    /// document (`_dakera_document`, Dakera v0.12.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<super::document::Citation>,
}

/// One neighbourhood-context entry of `dk memory recall`.
#[derive(Debug, Serialize)]
pub struct ContextRow {
    /// Memory id.
    pub id: String,
    /// Memory content (stored data, shown as is).
    pub content: String,
    /// The id of the ranked result this memory is context for (absent when
    /// the server does not say: inline context, or a server before v0.12.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_of: Option<String>,
    /// The 1-based place of that result in `memories`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_of_rank: Option<usize>,
    /// The raw similarity of the neighbourhood search: not comparable with
    /// the ranked results' scores.
    pub score: f32,
    /// Where the memory comes from, when it is a document chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<super::document::Citation>,
}

/// What `dk memory recall` shows: at most `--top-k` ranked results, and the
/// neighbourhood context apart from them.
#[derive(Debug, Serialize)]
pub struct RecallView {
    /// The ranked results, at most `--top-k`.
    pub memories: Vec<RecallRow>,
    /// The neighbourhood context of those results.
    pub context_memories: Vec<ContextRow>,
    /// Context items an older server appended to `memories` that were left
    /// out (under `--neighborhood separate` / `off`). Not printed in JSON.
    #[serde(skip)]
    pub omitted: usize,
}

/// The body of `POST /v1/memory/recall`: the SDK request's fields plus
/// `neighborhood_mode` (and `neighborhood: false` for `off`, which servers
/// before `neighborhood_mode` honour) and `include_summaries` when given.
pub fn recall_body(
    request: &RecallRequest,
    mode: &str,
    include_summaries: Option<bool>,
) -> Result<serde_json::Value> {
    let mut body = serde_json::to_value(request)?;
    body["neighborhood_mode"] = serde_json::json!(mode);
    if mode == "off" {
        body["neighborhood"] = serde_json::json!(false);
    }
    if let Some(include) = include_summaries {
        body["include_summaries"] = serde_json::json!(include);
    }
    Ok(body)
}

/// One recall result as a row, read with the SDK's result parser (which
/// takes the server's `{memory, score, ...}` items and flat ones).
fn recalled(item: &serde_json::Value) -> Result<dakera_client::memory::RecalledMemory> {
    serde_json::from_value(item.clone())
        .map_err(|e| anyhow::anyhow!("Failed to parse a recall result: {e}"))
}

fn recall_row(item: &serde_json::Value) -> Result<RecallRow> {
    let m = recalled(item)?;
    let matched_ids = item.get("matched_ids").and_then(|v| {
        v.as_array().map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
    });
    Ok(RecallRow {
        id: m.id,
        content: m.content,
        memory_type: memory_type_to_string(&m.memory_type),
        importance: m.importance,
        score: m.score,
        matched_text: api::str_of(item, "matched_text").map(str::to_string),
        matched_ids,
        document: super::document::citation(item),
    })
}

/// Split a recall answer into ranked results (at most `top_k`) and context.
/// Inline neighbourhood items (`retrieval: "neighborhood"`) and the
/// `context_memories` of `separate` mode become context. A server before
/// `neighborhood_mode` appends its context to `memories` unmarked: past
/// `top_k` it is context under `inline` and left out otherwise.
pub fn shape_recall(answer: &serde_json::Value, top_k: usize, mode: &str) -> Result<RecallView> {
    let empty = Vec::new();
    let items = answer
        .get("memories")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let (context_items, mut ranked_items): (Vec<&serde_json::Value>, Vec<&serde_json::Value>) =
        items
            .iter()
            .partition(|item| api::str_of(item, "retrieval") == Some("neighborhood"));
    let mut context_items = context_items;
    let overflow = if ranked_items.len() > top_k {
        ranked_items.split_off(top_k)
    } else {
        Vec::new()
    };
    let mut omitted = 0;
    if mode == "inline" {
        context_items.extend(overflow);
    } else {
        omitted = overflow.len();
    }
    if let Some(separate) = answer.get("context_memories").and_then(|v| v.as_array()) {
        context_items.extend(separate.iter());
    }
    let memories = ranked_items
        .into_iter()
        .map(recall_row)
        .collect::<Result<Vec<_>>>()?;
    let mut context_memories = Vec::with_capacity(context_items.len());
    for item in context_items {
        let m = recalled(item)?;
        let context_of = api::str_of(item, "context_of").map(str::to_string);
        let context_of_rank = context_of
            .as_deref()
            .and_then(|id| memories.iter().position(|r| r.id == id))
            .map(|i| i + 1);
        context_memories.push(ContextRow {
            id: m.id,
            content: m.content,
            context_of,
            context_of_rank,
            score: m.score,
            document: super::document::citation(item),
        });
    }
    Ok(RecallView {
        memories,
        context_memories,
        omitted,
    })
}

/// Print a recall: the ranked table (with a `matched_text` column when a
/// result has one, and a `source` column citing document chunks), then a
/// separate Context section. JSON / compact print
/// `{"memories": [...], "context_memories": [...]}` and nothing else on
/// stdout; the notes go to stderr there.
pub fn print_recall(view: &RecallView, format: crate::OutputFormat) {
    let table = matches!(format, crate::OutputFormat::Table);
    if view.omitted > 0 {
        let note = format!(
            "{} neighbourhood context item(s) left out: this server predates neighborhood_mode \
             and returns them inline; only the --top-k ranked results are shown",
            view.omitted
        );
        if table {
            output::warning(&note);
        } else {
            eprintln!("warning: {note}");
        }
    }
    if !table {
        output::print_item(view, format);
        return;
    }
    if view.memories.is_empty() {
        output::info("No memories found");
    } else {
        output::info(&found_message(view.memories.len(), 0));
        let matched = view.memories.iter().any(|r| r.matched_text.is_some());
        let cited = view.memories.iter().any(|r| r.document.is_some());
        let rows: Vec<serde_json::Value> = view
            .memories
            .iter()
            .map(|r| {
                let mut row = serde_json::json!({
                    "id": r.id,
                    "content": r.content,
                    "memory_type": r.memory_type,
                    "importance": r.importance,
                    "score": r.score,
                });
                if matched {
                    row["matched_text"] = serde_json::json!(r.matched_text);
                }
                if cited {
                    row["source"] = serde_json::json!(r.document.as_ref().map(|d| d.line()));
                }
                row
            })
            .collect();
        output::print_data(&rows, format);
    }
    if view.context_memories.is_empty() {
        return;
    }
    println!();
    output::info(&format!(
        "Context: {} session neighbour(s) of the results above (not ranked, not counted in \
         --top-k; scores come from another search)",
        view.context_memories.len()
    ));
    let cited = view.context_memories.iter().any(|c| c.document.is_some());
    let rows: Vec<serde_json::Value> = view
        .context_memories
        .iter()
        .map(|c| {
            let context_of = match (c.context_of_rank, c.context_of.as_deref()) {
                (Some(rank), Some(id)) => format!("#{rank} {id}"),
                (None, Some(id)) => id.to_string(),
                _ => "-".to_string(),
            };
            let mut row = serde_json::json!({
                "id": c.id,
                "content": c.content,
                "context_of": context_of,
                "score": c.score,
            });
            if cited {
                row["source"] = serde_json::json!(c.document.as_ref().map(|d| d.line()));
            }
            row
        })
        .collect();
    output::print_data(&rows, format);
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

/// The items of `dk memory batch-store`: `--content` values first, then the
/// `--file` JSON array (strings or item objects). `--type`, `--importance`,
/// `--tag` and `--session-id` fill in what an item does not set.
fn batch_store_items(
    sub: &ArgMatches,
    file_json: Option<&str>,
) -> Result<Vec<BatchStoreMemoryItem>> {
    let mut raw: Vec<serde_json::Value> = sub
        .get_many::<String>("content")
        .map(|v| v.map(|c| serde_json::json!(c)).collect())
        .unwrap_or_default();
    if let Some(text) = file_json {
        let parsed: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| input_error(format!("--file is not valid JSON: {e}")))?;
        match parsed {
            serde_json::Value::Array(items) => raw.extend(items),
            _ => return Err(input_error("--file must hold a JSON array of memories")),
        }
    }
    if raw.is_empty() {
        return Err(input_error(
            "nothing to store: give --content and/or --file",
        ));
    }
    if raw.len() > 1000 {
        return Err(input_error(format!(
            "{} memories given; the server accepts at most 1000 per batch",
            raw.len()
        )));
    }
    let memory_type = sub.get_one::<String>("type");
    let importance = sub.get_one::<f32>("importance");
    let tags: Option<Vec<String>> = sub.get_many::<String>("tag").map(|v| v.cloned().collect());
    let session = sub.get_one::<String>("session-id");
    raw.into_iter()
        .enumerate()
        .map(|(i, v)| {
            let mut obj = match v {
                serde_json::Value::String(content) => serde_json::json!({ "content": content }),
                serde_json::Value::Object(_) => v,
                _ => {
                    return Err(input_error(format!(
                        "item {i}: expected a string or an object"
                    )))
                }
            };
            if obj.get("content").and_then(|c| c.as_str()).is_none() {
                return Err(input_error(format!(
                    "item {i}: `content` (a string) is required"
                )));
            }
            if let Some(mt) = memory_type {
                if obj.get("memory_type").is_none() {
                    obj["memory_type"] = serde_json::json!(mt);
                }
            }
            if obj.get("importance").is_none() {
                obj["importance"] = serde_json::json!(importance.copied().unwrap_or(0.5));
            }
            if let Some(t) = &tags {
                if obj.get("tags").is_none() {
                    obj["tags"] = serde_json::json!(t);
                }
            }
            if let Some(sid) = session {
                if obj.get("session_id").is_none() {
                    obj["session_id"] = serde_json::json!(sid);
                }
            }
            serde_json::from_value::<BatchStoreMemoryItem>(obj)
                .map_err(|e| input_error(format!("item {i} is not a valid memory: {e}")))
        })
        .collect()
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
            if let Some(lang) = sub_matches.get_one::<String>("lang") {
                request = request.with_lang(lang);
            }
            if let Some(reference) = sub_matches.get_one::<String>("attachment-ref") {
                request = request.with_attachment_ref(reference);
            }

            let t = ctx.log_request("POST", "/v1/memory/store");
            let response = client.store_memory(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            // The store answer carries no namespace (the SDK's `namespace` is a
            // placeholder); the memory lives in the agent's memory namespace.
            output::success(&format!(
                "Memory stored (id: {}, agent: {agent_id})",
                response.memory_id
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
            request.lang = sub_matches.get_one::<String>("lang").cloned();
            let mode = sub_matches
                .get_one::<String>("neighborhood")
                .map(String::as_str)
                .unwrap_or("separate");
            let include_summaries = sub_matches.get_one::<bool>("include-summaries").copied();
            let body = recall_body(&request, mode, include_summaries)?;

            // Raw request: the `neighborhood_mode` / `include_summaries` fields
            // and the `context_memories` / `matched_text` answer fields are newer
            // than the SDK's recall types.
            let answer =
                api::request_json_logged(ctx, Method::POST, "/v1/memory/recall", Some(&body))
                    .await?;
            let shaped = shape_recall(&answer, top_k, mode)?;
            print_recall(&shaped, ctx.format);
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
            if let Some(lang) = sub_matches.get_one::<String>("lang") {
                body["lang"] = serde_json::json!(lang);
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
            request.lang = sub_matches.get_one::<String>("lang").cloned();

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
                output::info(&found_message(
                    response.memories.len(),
                    response.total_found,
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

        Some(("batch-store", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let file_json = match sub_matches.get_one::<String>("file") {
                Some(path) if path == "-" => {
                    let mut text = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                        .map_err(|e| input_error(format!("failed to read stdin: {e}")))?;
                    Some(text)
                }
                Some(path) => Some(
                    std::fs::read_to_string(path)
                        .map_err(|e| input_error(format!("failed to read file {path}: {e}")))?,
                ),
                None => None,
            };
            let items = batch_store_items(sub_matches, file_json.as_deref())?;
            let mut request = BatchStoreMemoryRequest::new(agent_id.clone(), items);
            request.lang = sub_matches.get_one::<String>("lang").cloned();

            let t = ctx.log_request("POST", "/v1/memories/store/batch");
            let response = client.store_memories_batch(request).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::success(&format!(
                "Stored {} memories (embedding: {} ms)",
                response.stored_count, response.total_embedding_time_ms
            ));
            let rows: Vec<StoredRow> = response
                .stored
                .into_iter()
                .map(|m| StoredRow {
                    id: m.id,
                    content: m.content,
                    importance: m.importance,
                })
                .collect();
            output::print_data(&rows, ctx.format);
        }

        Some(("extract", sub_matches)) => {
            let text = sub_matches.get_one::<String>("text").unwrap();
            let entity_types: Option<Vec<String>> =
                sub_matches.get_many::<String>("entity-types").map(|v| {
                    v.map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                });
            let lang = sub_matches.get_one::<String>("lang").map(String::as_str);

            let t = ctx.log_request("POST", "/v1/memories/extract");
            let response = client
                .extract_entities_with_lang(text, entity_types, lang)
                .await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.entities.is_empty() {
                output::info("No entities found");
            } else {
                output::info(&format!("Found {} entities", response.entities.len()));
                let rows: Vec<EntityRow> = response
                    .entities
                    .into_iter()
                    .map(|e| EntityRow {
                        entity_type: e.entity_type,
                        value: e.value,
                        score: e.score,
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
    fn found_message_shows_a_total_only_when_it_is_larger() {
        assert_eq!(found_message(1, 0), "Found 1 memories");
        assert_eq!(found_message(2, 2), "Found 2 memories");
        assert_eq!(found_message(2, 9), "Found 2 memories (total: 9)");
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
