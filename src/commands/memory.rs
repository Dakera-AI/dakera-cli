//! Memory management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::memory::{
    BatchMemoryFilter, BatchRecallRequest, BatchStoreMemoryItem, BatchStoreMemoryRequest,
    MemoryType, RecallRequest, StoreMemoryRequest, UpdateImportanceRequest,
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
    /// The ranking score: the server's `rank_score` (Dakera v0.12.3, the field
    /// that follows the order of the results), else `smart_score`, else
    /// `weighted_score`, else `score`.
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

/// A memory of a recall answer's `associated_memories` (knowledge-graph
/// neighbours of the results) or `instructions` (the agent's standing
/// instructions that fit the query), Dakera v0.12.4.
#[derive(Debug, Serialize)]
pub struct ExtraRow {
    /// Memory id.
    pub id: String,
    /// Memory content (stored data, shown as is).
    pub content: String,
    /// The score the server gave it (not comparable with the ranked scores).
    pub score: f32,
}

/// What `dk memory recall` shows: at most `--top-k` ranked results, and the
/// neighbourhood context apart from them.
#[derive(Debug, Serialize)]
pub struct RecallView {
    /// The ranked results, at most `--top-k`.
    pub memories: Vec<RecallRow>,
    /// The neighbourhood context of those results.
    pub context_memories: Vec<ContextRow>,
    /// Knowledge-graph neighbours of the results (shared entities,
    /// similarity, explicit links), not ranked: on by default since Dakera
    /// v0.12.4 (`--include-associated=false` turns them off).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub associated_memories: Vec<ExtraRow>,
    /// The agent's standing instructions that best fit the query (memories
    /// tagged `dakera-instruction`, at most 3; Dakera v0.12.4).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<ExtraRow>,
    /// Parts of the recall the server could not serve, each with why
    /// (Dakera v0.12.4). Printed as warnings; empty when it is complete.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub degraded: Vec<String>,
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
    let mut body = api::body_of(request)?;
    body["neighborhood_mode"] = serde_json::json!(mode);
    if mode == "off" {
        body["neighborhood"] = serde_json::json!(false);
    }
    if let Some(include) = include_summaries {
        body["include_summaries"] = serde_json::json!(include);
    }
    Ok(body)
}

/// `--include-associated[=BOOL]`, sent only when given: the server's default
/// (on since Dakera v0.12.4) applies otherwise.
pub fn set_include_associated(body: &mut serde_json::Value, include: Option<bool>) {
    if let Some(include) = include {
        body["include_associated"] = serde_json::json!(include);
    }
}

/// The memories of `answer[key]` (`associated_memories`, `instructions`).
fn extra_rows(answer: &serde_json::Value, key: &str) -> Result<Vec<ExtraRow>> {
    let empty = Vec::new();
    answer
        .get(key)
        .and_then(|v| v.as_array())
        .unwrap_or(&empty)
        .iter()
        .map(|item| {
            let m = recalled(item)?;
            Ok(ExtraRow {
                id: m.id,
                content: m.content,
                score: m.score,
            })
        })
        .collect()
}

/// One recall result as a row, read with the SDK's result parser (which
/// takes the server's `{memory, score, ...}` items and flat ones).
fn recalled(item: &serde_json::Value) -> Result<dakera_client::memory::RecalledMemory> {
    serde_json::from_value(item.clone())
        .map_err(|e| anyhow::anyhow!("Failed to parse a recall result: {e}"))
}

/// The score a ranked result is listed with. Since v0.12.3 the server sends
/// `rank_score`, which never rises down `memories`; `score` and `smart_score`
/// can, because later stages (the precision rerank, the per-session cap, the
/// temporal supplement) reorder the results after they are scored. Older
/// servers send no `rank_score`: `fallback` is the SDK's pick then.
fn ranking_score(item: &serde_json::Value, fallback: f32) -> f32 {
    item.get("rank_score")
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32)
        .unwrap_or(fallback)
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
        score: ranking_score(item, m.score),
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
    let degraded = answer
        .get("degraded")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|d| d.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(RecallView {
        memories,
        context_memories,
        associated_memories: extra_rows(answer, "associated_memories")?,
        instructions: extra_rows(answer, "instructions")?,
        degraded,
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
    for part in &view.degraded {
        let note = format!("the server could not serve part of this recall: {part}");
        if table {
            output::warning(&note);
        } else {
            eprintln!("warning: {note}");
        }
    }
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
    print_extras(
        &view.instructions,
        "Standing instructions: rules the user set earlier that fit this query (memories tagged \
         dakera-instruction)",
        format,
    );
    print_extras(
        &view.associated_memories,
        "Associated: knowledge-graph neighbours of the results (not ranked, not counted in \
         --top-k)",
        format,
    );
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

/// A titled section of extra recall memories (table output only).
fn print_extras(rows: &[ExtraRow], title: &str, format: crate::OutputFormat) {
    if rows.is_empty() {
        return;
    }
    println!();
    output::info(&format!("{title}: {}", rows.len()));
    output::print_data(rows, format);
}

/// Tell the user something about a successful answer without breaking JSON
/// output: a warning line in the table output, `warning: ...` on stderr
/// otherwise.
pub fn note(format: crate::OutputFormat, message: &str) {
    if matches!(format, crate::OutputFormat::Table) {
        output::warning(message);
    } else {
        eprintln!("warning: {message}");
    }
}

/// What to say about a stored text the server read as a request to forget
/// (`forget_request`, Dakera v0.12.4): how to act on it.
pub fn forget_request_note(agent_id: &str, intent: &serde_json::Value) -> Option<String> {
    if !api::bool_of(intent, "is_request").unwrap_or(false) {
        return None;
    }
    let probability = intent
        .get("probability")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NAN);
    Some(format!(
        "this text reads as a request to forget something said (probability {probability:.2}). \
         To forget what it means: dk memory forget {agent_id} --request '<the text>' (unless the \
         server runs DAKERA_FORGET_REQUESTS=apply, which does it itself, restorably)"
    ))
}

/// The body of `POST /v1/memory/consolidate` from `dk memory consolidate`'s
/// arguments: the server reads `memory_ids`, `threshold` and `target_type`
/// only. `--type` and `--dry-run` were sent before 0.8.3 and the server
/// ignored them: a "dry run" merged.
pub fn consolidate_body(agent_id: &str, sub: &ArgMatches) -> Result<serde_json::Value> {
    if sub.get_flag("dry-run") {
        return Err(input_error(
            "the server has no consolidation preview, so --dry-run is refused (before dk 0.8.3 \
             it was sent, ignored by the server, and the memories were merged). Use --ids to \
             choose the memories to merge",
        ));
    }
    if sub.get_one::<String>("type").is_some() {
        return Err(input_error(
            "--type is refused: the server never filtered consolidation by type (it merges \
             memories of one type); --target-type sets the consolidated memory's type",
        ));
    }
    let mut body = serde_json::json!({ "agent_id": agent_id });
    if let Some(ids) = sub.get_many::<String>("ids") {
        let ids: Vec<&str> = ids.map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if !ids.is_empty() {
            body["memory_ids"] = serde_json::json!(ids);
        }
    }
    if let Some(threshold) = sub.get_one::<f64>("threshold") {
        if !(*threshold > 0.0 && *threshold <= 1.0) {
            return Err(input_error(format!(
                "--threshold is a cosine similarity in (0, 1], got {threshold}"
            )));
        }
        body["threshold"] = serde_json::json!(threshold);
    }
    if let Some(t) = sub.get_one::<String>("target-type") {
        body["target_type"] = serde_json::json!(t);
    }
    Ok(body)
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

/// The stored memories of a batch store answer.
fn stored_rows(answer: &serde_json::Value) -> Vec<StoredRow> {
    answer
        .get("stored")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .map(|m| StoredRow {
            id: api::str_of(m, "id").unwrap_or("").to_string(),
            content: api::str_of(m, "content").unwrap_or("").to_string(),
            importance: m.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        })
        .collect()
}

/// What to say about a batch store answer (Dakera v0.12.4): items not stored
/// as near-duplicates (`duplicates`, under the namespace's `dedup_on_store`)
/// and stored texts that read as requests to forget (`forget_requests`).
pub fn batch_store_notes(agent_id: &str, answer: &serde_json::Value) -> Vec<String> {
    let mut notes = Vec::new();
    let empty = Vec::new();
    let list = |key: &str| {
        answer
            .get(key)
            .and_then(|v| v.as_array())
            .unwrap_or(&empty)
            .clone()
    };
    let duplicates = list("duplicates");
    if !duplicates.is_empty() {
        let items: Vec<String> = duplicates
            .iter()
            .map(|d| {
                format!(
                    "#{} = {}",
                    api::u64_of(d, "index")
                        .map(|i| i.to_string())
                        .unwrap_or_default(),
                    api::str_of(d, "existing_id").unwrap_or("?")
                )
            })
            .collect();
        notes.push(format!(
            "{} item(s) not stored: the namespace deduplicates on store and each is a \
             near-duplicate of a stored memory (item = existing id): {}",
            duplicates.len(),
            items.join(", ")
        ));
    }
    let requests = list("forget_requests");
    let ids: Vec<&str> = requests
        .iter()
        .filter(|r| api::bool_of(r, "is_request").unwrap_or(true))
        .filter_map(|r| api::str_of(r, "id"))
        .collect();
    if !ids.is_empty() {
        notes.push(format!(
            "{} stored memory(ies) read as a request to forget something said ({}). To forget \
             what one means: dk memory forget {agent_id} --request '<its text>' (unless the \
             server runs DAKERA_FORGET_REQUESTS=apply, which does it itself)",
            ids.len(),
            ids.join(", ")
        ));
    }
    notes
}

/// The entities of a `POST /v1/memories/extract` answer.
fn entity_rows(answer: &serde_json::Value) -> Vec<EntityRow> {
    answer
        .get("entities")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .map(|e| EntityRow {
            entity_type: api::str_of(e, "entity_type").unwrap_or("").to_string(),
            value: api::str_of(e, "value").unwrap_or("").to_string(),
            score: e.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
        })
        .collect()
}

/// `dk memory get --projection-days`: the memory, then its projected
/// importance per day (table), or the whole answer (JSON).
fn print_projection(answer: &serde_json::Value, format: crate::OutputFormat) {
    if !matches!(format, crate::OutputFormat::Table) {
        output::print_item(answer, format);
        return;
    }
    let mut memory = answer.clone();
    let projection = memory
        .as_object_mut()
        .and_then(|m| m.remove("strength_projection"))
        .unwrap_or(serde_json::Value::Null);
    output::print_item(&memory, format);
    let points: Vec<serde_json::Value> = projection
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|p| serde_json::json!({ "day": p.get("day"), "importance": p.get("importance") }))
        .collect();
    if points.is_empty() {
        output::info("The server sent no strength projection (it needs v0.12.4)");
        return;
    }
    println!();
    output::info("Projected importance if nothing touches the memory (day 0 = today)");
    output::print_data(&points, format);
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

            // Raw request: the answer's `forget_request` (v0.12.4) and
            // `session_state` are newer than the SDK's store response.
            let body = api::body_of(&request)?;
            let answer =
                api::request_json_logged(ctx, Method::POST, "/v1/memory/store", Some(&body))
                    .await?;
            let id = answer
                .pointer("/memory/id")
                .or_else(|| answer.get("memory_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");

            // The store answer carries no namespace; the memory lives in the
            // agent's memory namespace.
            if matches!(ctx.format, crate::OutputFormat::Table) {
                output::success(&format!("Memory stored (id: {id}, agent: {agent_id})"));
            } else {
                output::print_item(&answer, ctx.format);
            }
            if api::str_of(&answer, "session_state") == Some("ended") {
                note(
                    ctx.format,
                    "the session has ended (the memory was stored all the same); start a new \
                     session for what follows",
                );
            }
            if let Some(text) = answer
                .get("forget_request")
                .and_then(|i| forget_request_note(agent_id, i))
            {
                note(ctx.format, &text);
            }
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
            let mut body = recall_body(&request, mode, include_summaries)?;
            set_include_associated(
                &mut body,
                sub_matches.get_one::<bool>("include-associated").copied(),
            );

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

            if let Some(days) = sub_matches.get_one::<u32>("projection-days") {
                let path = format!(
                    "/v1/memory/get/{}?agent_id={}&projection_days={days}",
                    api::segment(memory_id),
                    api::segment(agent_id)
                );
                let answer = api::request_json_logged(ctx, Method::GET, &path, None).await?;
                print_projection(&answer, ctx.format);
                return Ok(());
            }
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

        Some((name @ ("forget" | "forget-detect" | "trash" | "restore"), sub_matches)) => {
            super::forget::execute(ctx, name, sub_matches).await?;
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

            // Raw request: `duplicates` and `forget_requests` (v0.12.4) are
            // newer than the SDK's batch response.
            let body = api::body_of(&request)?;
            let path = "/v1/memories/store/batch";
            let answer = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            let table = matches!(ctx.format, crate::OutputFormat::Table);
            if table {
                output::success(&format!(
                    "Stored {} memories (embedding: {} ms)",
                    api::u64_of(&answer, "stored_count").unwrap_or(0),
                    api::u64_of(&answer, "total_embedding_time_ms").unwrap_or(0)
                ));
            }
            let rows = stored_rows(&answer);
            output::print_data(&rows, ctx.format);
            for text in batch_store_notes(agent_id, &answer) {
                note(ctx.format, &text);
            }
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

            // Raw request: the answer's `extractor` and `warnings` (v0.12.4)
            // are newer than the SDK's extract response.
            let mut body = serde_json::json!({ "content": text });
            if let Some(types) = entity_types {
                body["entity_types"] = serde_json::json!(types);
            }
            if let Some(lang) = lang {
                body["lang"] = serde_json::json!(lang);
            }
            let path = "/v1/memories/extract";
            let answer = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            for warning in answer
                .get("warnings")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|w| w.as_str())
            {
                note(ctx.format, warning);
            }
            let rows = entity_rows(&answer);
            if rows.is_empty() {
                output::info("No entities found");
            } else {
                if matches!(ctx.format, crate::OutputFormat::Table) {
                    let by = match api::str_of(&answer, "extractor") {
                        Some("gliner") => " (GLiNER model and rules)",
                        Some("rule_based") => " (rules only: dates, URLs, emails, ...)",
                        _ => "",
                    };
                    output::info(&format!("Found {} entities{by}", rows.len()));
                }
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
            let body = consolidate_body(agent_id, sub_matches)?;
            let path = "/v1/memory/consolidate";
            let answer = api::request_json_logged(ctx, Method::POST, path, Some(&body)).await?;
            if matches!(ctx.format, crate::OutputFormat::Table) {
                let sources = answer
                    .get("source_memory_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let merged_into = answer
                    .pointer("/consolidated_memory/id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                output::success(&format!(
                    "Merged {sources} memories into '{merged_into}' ({} removed; `dk memory trash \
                     {agent_id}` lists them while they can be restored)",
                    api::u64_of(&answer, "memories_removed").unwrap_or(0)
                ));
            }
            output::print_item(&answer, ctx.format);
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

    fn ranked(id: &str, score: f64, smart: f64, rank: Option<f64>) -> serde_json::Value {
        let mut item = serde_json::json!({
            "memory": {
                "id": id, "content": "c", "memory_type": "episodic", "importance": 0.5,
                "agent_id": "a", "tags": [], "created_at": 1, "last_accessed_at": 1,
                "access_count": 0
            },
            "score": score,
            "smart_score": smart,
        });
        if let Some(r) = rank {
            item["rank_score"] = serde_json::json!(r);
        }
        item
    }

    /// v0.12.3 (#940): the listed score is `rank_score`, which follows the
    /// order of the results, while `score` / `smart_score` may rise down it.
    #[test]
    fn recall_lists_rank_score_when_the_server_sends_it() {
        let answer = serde_json::json!({"memories": [
            ranked("m1", 0.2, 0.5, Some(0.9)),
            ranked("m2", 0.8, 0.7, Some(0.6)),
        ]});
        let view = shape_recall(&answer, 10, "separate").unwrap();
        let scores: Vec<f32> = view.memories.iter().map(|r| r.score).collect();
        assert_eq!(scores, vec![0.9, 0.6]);
    }

    /// A server before v0.12.3 sends no `rank_score`: the SDK's pick
    /// (`smart_score` first) is listed, as before.
    #[test]
    fn recall_without_rank_score_lists_smart_score() {
        let answer = serde_json::json!({"memories": [ranked("m1", 0.2, 0.5, None)]});
        let view = shape_recall(&answer, 10, "separate").unwrap();
        assert_eq!(view.memories[0].score, 0.5);
    }
}
