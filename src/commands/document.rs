//! `dk document` — document import (Dakera v0.12.3, Dakera-AI/dakera#923).
//!
//! A document becomes chunk memories of an agent (heading breadcrumb, page,
//! provenance in `_dakera_document`), written by a background job.
//!
//! * `import` text / Markdown / HTML: `POST /v1/memory/documents`;
//!   PDF / DOCX (or text that is not UTF-8): `POST /v1/namespaces/{ns}/attachments`,
//!   then `POST /v1/namespaces/{ns}/attachments/{ref}/document`
//! * `job`    `GET    /v1/memory/documents/jobs/{job_id}?agent_id=`
//! * `list`   `GET    /v1/memory/documents?agent_id=&cursor=&limit=`
//! * `show`   `GET    /v1/memory/documents/{document_id}?agent_id=`
//! * `delete` `DELETE /v1/memory/documents/{document_id}?agent_id=`

use std::io::Read;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};

use crate::api::{self, str_of, u64_of, ApiHttpError};
use crate::context::Context;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The media types the text route accepts.
pub const TEXT_MEDIA_TYPES: &[&str] = &["text/plain", "text/markdown", "text/html"];

/// The DOCX media type.
pub const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// What a missing document route means.
const NEEDS_SERVER: &str = "document import needs a Dakera v0.12.3 server: this server has no \
     /v1/memory/documents route (`dk health` shows its version)";

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let result = match matches.subcommand() {
        Some(("import", sub)) => import(ctx, sub).await,
        Some(("job", sub)) => job(ctx, sub).await,
        Some(("list", sub)) => list(ctx, sub).await,
        Some(("show", sub)) => show(ctx, sub).await,
        Some(("delete", sub)) => delete(ctx, sub).await,
        _ => {
            output::error("Unknown document subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    };
    result.map_err(explain_missing_route)
}

/// A `404 ROUTE_NOT_FOUND` / `405` / bodiless `404`: the server predates the
/// document routes. Other errors pass unchanged.
fn explain_missing_route(err: anyhow::Error) -> anyhow::Error {
    let missing = match err.downcast_ref::<ApiHttpError>() {
        Some(e) => match e.status {
            405 => true,
            404 => matches!(e.code.as_deref(), None | Some("ROUTE_NOT_FOUND")),
            _ => false,
        },
        None => false,
    };
    if missing {
        err.context(NEEDS_SERVER)
    } else {
        err
    }
}

/// The media type of a document file, from its extension; `None` when the
/// extension says nothing usable.
pub fn media_type_for(path: &str) -> Option<&'static str> {
    if path == "-" {
        return Some("text/plain");
    }
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "text" | "log" => Some("text/plain"),
        "md" | "markdown" | "mdown" | "mkd" => Some("text/markdown"),
        "html" | "htm" => Some("text/html"),
        "xhtml" => Some("application/xhtml+xml"),
        "pdf" => Some("application/pdf"),
        "docx" => Some(DOCX),
        _ => None,
    }
}

/// Which route imports a document: the text route takes UTF-8 text, Markdown
/// and HTML; anything else (PDF, DOCX, XHTML, text that is not UTF-8) is
/// uploaded as an attachment and imported from there.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// `POST /v1/memory/documents` with this `media_type` and content.
    Text { media_type: String, content: String },
    /// Upload with this `Content-Type`, then import the attachment.
    Attachment { media_type: String, bytes: Vec<u8> },
}

/// Choose the route for `bytes` of `media_type`.
pub fn route_for(media_type: &str, bytes: Vec<u8>) -> Route {
    if TEXT_MEDIA_TYPES.contains(&media_type) {
        match String::from_utf8(bytes) {
            Ok(content) => {
                return Route::Text {
                    media_type: media_type.to_string(),
                    content,
                }
            }
            // UTF-16 or Windows-1252: the server decodes it from the bytes.
            Err(e) => {
                return Route::Attachment {
                    media_type: media_type.to_string(),
                    bytes: e.into_bytes(),
                }
            }
        }
    }
    Route::Attachment {
        media_type: media_type.to_string(),
        bytes,
    }
}

/// The fields both import routes share, from the flags: only what was given
/// (the server refuses unknown `chunking` fields).
pub fn common_body(sub: &ArgMatches, agent_id: &str) -> Value {
    let mut body = json!({ "agent_id": agent_id });
    for (flag, field) in [
        ("title", "title"),
        ("document-id", "document_id"),
        ("session-id", "session_id"),
        ("type", "memory_type"),
        ("lang", "lang"),
    ] {
        if let Some(v) = sub.get_one::<String>(flag) {
            body[field] = json!(v);
        }
    }
    if let Some(v) = sub.get_one::<f32>("importance") {
        body["importance"] = json!(v);
    }
    if let Some(tags) = sub.get_many::<String>("tag") {
        let tags: Vec<&String> = tags.collect();
        body["tags"] = json!(tags);
    }
    if sub.get_flag("decompose") {
        body["decompose"] = json!(true);
    }
    let mut chunking = serde_json::Map::new();
    if let Some(n) = sub.get_one::<u64>("chunk-tokens") {
        chunking.insert("max_tokens".into(), json!(n));
    }
    if let Some(n) = sub.get_one::<u64>("overlap") {
        chunking.insert("overlap_tokens".into(), json!(n));
    }
    if let Some(s) = sub.get_one::<String>("strategy") {
        chunking.insert("strategy".into(), json!(s));
    }
    if !chunking.is_empty() {
        body["chunking"] = Value::Object(chunking);
    }
    body
}

fn read_source(file: &str) -> Result<Vec<u8>> {
    if file == "-" {
        let mut data = Vec::new();
        std::io::stdin()
            .read_to_end(&mut data)
            .map_err(|e| input_error(format!("failed to read stdin: {e}")))?;
        return Ok(data);
    }
    std::fs::read(file).map_err(|e| input_error(format!("failed to read file {file}: {e}")))
}

/// `?agent_id=` with the id encoded.
fn agent_query(agent_id: &str) -> String {
    let mut query = String::from("agent_id=");
    query.push_str(&encode_query(agent_id));
    query
}

/// Percent-encode a query value.
fn encode_query(raw: &str) -> String {
    api::segment(raw).replace(':', "%3A")
}

/// The job-status route of an agent's import job.
pub fn job_path(agent_id: &str, job_id: &str) -> String {
    let job = api::segment(job_id);
    let query = agent_query(agent_id);
    format!("/v1/memory/documents/jobs/{job}?{query}")
}

/// The route of one document.
pub fn document_path(agent_id: &str, document_id: &str) -> String {
    let id = api::segment(document_id);
    let query = agent_query(agent_id);
    format!("/v1/memory/documents/{id}?{query}")
}

async fn import(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let file = sub.get_one::<String>("file").unwrap();
    let agent_id = sub.get_one::<String>("agent").unwrap();
    let media_type = match sub.get_one::<String>("media-type") {
        Some(m) => m.clone(),
        None => match media_type_for(file) {
            Some(m) => m.to_string(),
            None => {
                return Err(input_error(format!(
                    "cannot tell the format of {file} from its extension; pass --media-type \
                     (text/plain, text/markdown, text/html, application/pdf or {DOCX})"
                )))
            }
        },
    };
    let bytes = read_source(file)?;
    let mut body = common_body(sub, agent_id);
    let accepted = match route_for(&media_type, bytes) {
        Route::Text {
            media_type,
            content,
        } => {
            body["content"] = json!(content);
            body["media_type"] = json!(media_type);
            api::request_json_logged(ctx, Method::POST, "/v1/memory/documents", Some(&body)).await?
        }
        Route::Attachment { media_type, bytes } => {
            let namespace = match sub.get_one::<String>("namespace") {
                Some(ns) => ns.clone(),
                None => format!("_dakera_agent_{agent_id}"),
            };
            let ns = api::segment(&namespace);
            let upload = format!("/v1/namespaces/{ns}/attachments");
            let uploaded = api::post_bytes(ctx, &upload, &media_type, bytes).await?;
            let Some(reference) = str_of(&uploaded, "attachment_ref") else {
                anyhow::bail!("the attachment upload answered no attachment_ref: {uploaded}");
            };
            let reference = api::segment(reference);
            let path = format!("/v1/namespaces/{ns}/attachments/{reference}/document");
            api::request_json_logged(ctx, Method::POST, &path, Some(&body)).await?
        }
    };
    let table = matches!(ctx.format, OutputFormat::Table);
    let job_id = str_of(&accepted, "job_id").unwrap_or("").to_string();
    let document_id = str_of(&accepted, "document_id").unwrap_or("").to_string();
    if !sub.get_flag("wait") || job_id.is_empty() {
        if table {
            output::success(&format!(
                "Import of document '{document_id}' started (job {job_id})"
            ));
            output::info(&format!(
                "Follow it with `dk document job {job_id} --agent {agent_id}`"
            ));
        } else {
            output::print_item(&accepted, ctx.format);
        }
        return Ok(());
    }
    let timeout = sub.get_one::<u64>("timeout").copied().unwrap_or(600);
    let finished = wait_for_job(ctx, agent_id, &job_id, timeout).await?;
    let status = str_of(&finished, "status").unwrap_or("");
    if status != "Completed" {
        return Err(job_failed(&finished));
    }
    if table {
        let message = str_of(&finished, "message").unwrap_or("done");
        output::success(&format!("Document '{document_id}' imported: {message}"));
    } else {
        let out = json!({ "accepted": accepted, "job": finished });
        output::print_item(&out, ctx.format);
    }
    Ok(())
}

/// The error of a job that did not complete, with its status and code.
fn job_failed(job: &Value) -> anyhow::Error {
    let status = str_of(job, "status").unwrap_or("?");
    let message = str_of(job, "message").unwrap_or("");
    let code = api::ptr_str(job, "/error/code").unwrap_or("");
    let http = api::ptr_u64(job, "/error/status").unwrap_or(0);
    if http > 0 {
        anyhow::anyhow!("document import {status} ({http} {code}): {message}")
    } else {
        anyhow::anyhow!("document import {status}: {message}")
    }
}

/// Poll the job until it ends or `timeout_secs` pass.
async fn wait_for_job(
    ctx: &Context,
    agent_id: &str,
    job_id: &str,
    timeout_secs: u64,
) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let path = job_path(agent_id, job_id);
    loop {
        let job = api::request_json_logged(ctx, Method::GET, &path, None).await?;
        match str_of(&job, "status") {
            Some("Completed") | Some("Failed") | Some("Cancelled") => return Ok(job),
            _ => {}
        }
        if Instant::now() + POLL_INTERVAL >= deadline {
            let progress = u64_of(&job, "progress").unwrap_or(0);
            anyhow::bail!(
                "the import job {job_id} is still running after {timeout_secs}s ({progress}%); \
                 follow it with `dk document job {job_id} --agent {agent_id}`"
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn show_value(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

async fn job(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent").unwrap();
    let job_id = sub.get_one::<String>("job_id").unwrap();
    let job = api::request_json_logged(ctx, Method::GET, &job_path(agent_id, job_id), None).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&job, ctx.format);
        return Ok(());
    }
    let error = match (
        api::ptr_u64(&job, "/error/status"),
        api::ptr_str(&job, "/error/code"),
    ) {
        (Some(status), Some(code)) => format!("{status} {code}"),
        _ => "-".to_string(),
    };
    let progress = format!("{}%", u64_of(&job, "progress").unwrap_or(0));
    let pairs = vec![
        ("Job", show_value(job.get("id"))),
        ("Status", show_value(job.get("status"))),
        ("Progress", progress),
        ("Message", show_value(job.get("message"))),
        ("Document", show_value(job.pointer("/metadata/document_id"))),
        ("Error", error),
    ];
    output::print_kv(&pairs, ctx.format);
    Ok(())
}

/// One row of `dk document list`.
#[derive(Serialize)]
struct DocumentRow {
    document_id: String,
    title: String,
    media_type: String,
    chunks: u64,
    bytes: u64,
    pages: String,
    updated_at: u64,
}

fn document_row(d: &Value) -> DocumentRow {
    DocumentRow {
        document_id: str_of(d, "document_id").unwrap_or("").to_string(),
        title: str_of(d, "title").unwrap_or("-").to_string(),
        media_type: str_of(d, "media_type").unwrap_or("").to_string(),
        chunks: u64_of(d, "chunk_count").unwrap_or(0),
        bytes: u64_of(d, "bytes").unwrap_or(0),
        pages: show_value(d.get("pages")),
        updated_at: u64_of(d, "updated_at").unwrap_or(0),
    }
}

/// The listing route with its query.
pub fn list_path(agent_id: &str, limit: Option<u64>, cursor: Option<&str>) -> String {
    let mut path = format!("/v1/memory/documents?{}", agent_query(agent_id));
    if let Some(limit) = limit {
        path.push_str(&format!("&limit={limit}"));
    }
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.push_str(&encode_query(cursor));
    }
    path
}

async fn list(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent").unwrap();
    let limit = sub.get_one::<u64>("limit").copied();
    let cursor = sub.get_one::<String>("cursor").map(String::as_str);
    let path = list_path(agent_id, limit, cursor);
    let result = api::request_json_logged(ctx, Method::GET, &path, None).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    let empty = Vec::new();
    let documents = result
        .get("documents")
        .and_then(|d| d.as_array())
        .unwrap_or(&empty);
    if documents.is_empty() {
        output::info(&format!("No documents for agent '{agent_id}'"));
    } else {
        let rows: Vec<DocumentRow> = documents.iter().map(document_row).collect();
        output::print_data(&rows, ctx.format);
    }
    if let Some(next) = str_of(&result, "next_cursor") {
        output::info(&format!("More: --cursor {next}"));
    }
    Ok(())
}

/// One chunk row of `dk document show`.
#[derive(Serialize)]
struct ChunkRow {
    index: u64,
    id: String,
    pages: String,
    heading_path: String,
    chars: String,
}

/// A page range as `3` or `3-4`, `-` when the chunk has none.
pub fn page_range(start: Option<u64>, end: Option<u64>) -> String {
    match (start, end) {
        (Some(s), Some(e)) if e != s => format!("{s}-{e}"),
        (Some(s), _) => s.to_string(),
        (None, Some(e)) => e.to_string(),
        (None, None) => "-".to_string(),
    }
}

/// A heading path as one breadcrumb (`Manual › Install › Linux`).
pub fn breadcrumb(path: Option<&Value>) -> String {
    let parts: Vec<&str> = path
        .and_then(|p| p.as_array())
        .map(|items| items.iter().filter_map(|h| h.as_str()).collect())
        .unwrap_or_default();
    parts.join(" › ")
}

async fn show(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent").unwrap();
    let document_id = sub.get_one::<String>("document_id").unwrap();
    let path = document_path(agent_id, document_id);
    let detail = api::request_json_logged(ctx, Method::GET, &path, None).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&detail, ctx.format);
        return Ok(());
    }
    let doc = detail.get("document").cloned().unwrap_or(Value::Null);
    let pairs = vec![
        ("Document", show_value(doc.get("document_id"))),
        ("Title", show_value(doc.get("title"))),
        ("Media type", show_value(doc.get("media_type"))),
        ("Chunks", show_value(doc.get("chunk_count"))),
        ("Bytes", show_value(doc.get("bytes"))),
        ("Pages", show_value(doc.get("pages"))),
        ("SHA-256", show_value(doc.get("sha256"))),
        ("Attachment", show_value(doc.get("attachment_ref"))),
        ("Created at", show_value(doc.get("created_at"))),
        ("Updated at", show_value(doc.get("updated_at"))),
    ];
    output::print_kv(&pairs, ctx.format);
    let empty = Vec::new();
    let chunks = detail
        .get("chunks")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    if chunks.is_empty() {
        return Ok(());
    }
    println!();
    let rows: Vec<ChunkRow> = chunks
        .iter()
        .map(|c| ChunkRow {
            index: u64_of(c, "chunk_index").unwrap_or(0),
            id: str_of(c, "id").unwrap_or("").to_string(),
            pages: page_range(u64_of(c, "page_start"), u64_of(c, "page_end")),
            heading_path: breadcrumb(c.get("heading_path")),
            chars: format!(
                "{}-{}",
                u64_of(c, "char_start").unwrap_or(0),
                u64_of(c, "char_end").unwrap_or(0)
            ),
        })
        .collect();
    output::print_data(&rows, ctx.format);
    Ok(())
}

async fn delete(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent").unwrap();
    let document_id = sub.get_one::<String>("document_id").unwrap();
    let path = document_path(agent_id, document_id);
    let result = api::request_json_logged(ctx, Method::DELETE, &path, None).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&result, ctx.format);
        return Ok(());
    }
    let chunks = u64_of(&result, "deleted_chunks").unwrap_or(0);
    output::success(&format!(
        "Document '{document_id}' deleted ({chunks} chunk memories)"
    ));
    Ok(())
}

/// The citation of a recall result that is a document chunk: from
/// `memory.metadata._dakera_document` (flat results carry `metadata` at the
/// top level).
pub fn citation(item: &Value) -> Option<Citation> {
    let memory = item.get("memory").unwrap_or(item);
    let doc = memory.get("metadata")?.get("_dakera_document")?;
    let id = doc.get("id")?.as_str()?.to_string();
    let heading_path = doc
        .get("heading_path")
        .and_then(|p| p.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|h| h.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(Citation {
        document_id: id,
        title: str_of(doc, "title").map(str::to_string),
        page_start: u64_of(doc, "page_start"),
        page_end: u64_of(doc, "page_end"),
        heading_path,
        chunk_index: u64_of(doc, "chunk_index"),
    })
}

/// Where a recalled chunk comes from.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Citation {
    /// The document id.
    pub document_id: String,
    /// The document title, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// First page of the chunk's text (PDF, form-feed pages of text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_start: Option<u64>,
    /// Last page of the chunk's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_end: Option<u64>,
    /// Headings above the chunk, outermost first.
    pub heading_path: Vec<String>,
    /// Position of the chunk in the document, from 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_index: Option<u64>,
}

impl Citation {
    /// One line: `Field Manual, p. 39-40 › Install › Linux`.
    pub fn line(&self) -> String {
        let mut out = self
            .title
            .clone()
            .unwrap_or_else(|| self.document_id.clone());
        if self.page_start.is_some() || self.page_end.is_some() {
            out.push_str(&format!(
                ", p. {}",
                page_range(self.page_start, self.page_end)
            ));
        }
        // The title is the breadcrumb's root: do not repeat it.
        let skip = usize::from(self.heading_path.first() == self.title.as_ref());
        let rest: Vec<&str> = self.heading_path[skip.min(self.heading_path.len())..]
            .iter()
            .map(String::as_str)
            .collect();
        if !rest.is_empty() {
            out.push_str(" › ");
            out.push_str(&rest.join(" › "));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types_follow_the_extension() {
        assert_eq!(media_type_for("a.MD"), Some("text/markdown"));
        assert_eq!(media_type_for("a.htm"), Some("text/html"));
        assert_eq!(media_type_for("manual.pdf"), Some("application/pdf"));
        assert_eq!(media_type_for("x.docx"), Some(DOCX));
        assert_eq!(media_type_for("-"), Some("text/plain"));
        assert_eq!(media_type_for("x.bin"), None);
    }

    #[test]
    fn text_goes_to_the_text_route_and_the_rest_to_attachments() {
        let text = route_for("text/markdown", b"# T".to_vec());
        assert_eq!(
            text,
            Route::Text {
                media_type: "text/markdown".into(),
                content: "# T".into()
            }
        );
        assert!(matches!(
            route_for("application/pdf", b"%PDF".to_vec()),
            Route::Attachment { .. }
        ));
        // Not UTF-8: the server decodes the bytes itself.
        let latin = route_for("text/plain", vec![0x63, 0x61, 0x66, 0xE9]);
        assert!(matches!(latin, Route::Attachment { ref bytes, .. } if bytes.len() == 4));
    }

    #[test]
    fn paths_encode_ids() {
        assert_eq!(
            job_path("a b", "job_1"),
            "/v1/memory/documents/jobs/job_1?agent_id=a%20b"
        );
        assert_eq!(
            document_path("bot", "v3/manual"),
            "/v1/memory/documents/v3%2Fmanual?agent_id=bot"
        );
        assert_eq!(
            list_path("bot", Some(10), Some("m:1")),
            "/v1/memory/documents?agent_id=bot&limit=10&cursor=m%3A1"
        );
    }

    #[test]
    fn a_citation_names_title_pages_and_sections() {
        let item = json!({"memory": {"id": "doc_1", "metadata": {"_dakera_document": {
            "id": "manual", "title": "Field Manual", "page_start": 39, "page_end": 40,
            "heading_path": ["Field Manual", "Install", "Linux"], "chunk_index": 41
        }}}});
        let c = citation(&item).unwrap();
        assert_eq!(c.line(), "Field Manual, p. 39-40 › Install › Linux");
        let plain = json!({"memory": {"id": "m", "metadata": {"source": "x"}}});
        assert!(citation(&plain).is_none());
        let untitled = json!({"memory": {"metadata": {"_dakera_document": {
            "id": "notes", "heading_path": ["Setup"]
        }}}});
        assert_eq!(citation(&untitled).unwrap().line(), "notes › Setup");
    }
}
