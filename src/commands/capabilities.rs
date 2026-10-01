//! `dk capabilities` — what the connected server supports (`GET /v1/capabilities`).
//!
//! The document is new in Dakera v0.12. It names the active embedding model,
//! the search mode and scoring strategy, and which opt-in features are on
//! (attachments, speech to text, image indexing, records), so a script can
//! check before it depends on one. A v0.11 server has no such route.

use anyhow::Result;
use dakera_client::reqwest::Method;
use serde_json::Value;

use crate::api::{self, ApiHttpError};
use crate::context::Context;
use crate::output;
use crate::OutputFormat;

/// Render a JSON value for a table cell; a missing value is `-`.
pub fn show(doc: &Value, pointer: &str) -> String {
    match doc.pointer(pointer) {
        None | Some(Value::Null) => "-".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => {
            let parts: Vec<String> = items.iter().map(show_scalar).collect();
            parts.join(", ")
        }
        Some(other) => other.to_string(),
    }
}

fn show_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `enabled` / `disabled` for a boolean at `pointer`.
fn show_switch(doc: &Value, pointer: &str, variable: &str) -> String {
    match doc.pointer(pointer).and_then(|v| v.as_bool()) {
        Some(true) => "enabled".to_string(),
        Some(false) => format!("disabled (set {variable} to turn it on)"),
        None => "-".to_string(),
    }
}

/// The rows `dk capabilities` prints.
pub fn summary(doc: &Value) -> Vec<(&'static str, String)> {
    let attachments = show_switch(doc, "/attachments/enabled", "DAKERA_ATTACHMENTS");
    let vision = show_switch(doc, "/vision/enabled", "DAKERA_VISION");
    let records = show_switch(doc, "/records/enabled", "DAKERA_RECORDS");
    let max_bytes = show(doc, "/attachments/max_bytes");
    let speech = show(doc, "/attachments/transcription/model");
    vec![
        ("Server version", show(doc, "/server_version")),
        ("API versions", show(doc, "/api_versions")),
        ("Default model", show(doc, "/default_model")),
        ("Search mode", show(doc, "/search_mode")),
        ("Scoring strategy", show(doc, "/scoring/strategy")),
        ("Distance metrics", show(doc, "/distance_metrics")),
        ("Full-text language", show(doc, "/fulltext_language")),
        ("Query languages (lang)", show(doc, "/query_languages")),
        ("Attachments", attachments),
        ("Attachment max bytes", max_bytes),
        ("Speech to text model", speech),
        ("Image indexing", vision),
        ("Records", records),
        ("Re-embed pending", show(doc, "/reembed_pending")),
        ("Unreadable records", show(doc, "/unreadable_records")),
        ("On-disk format", show(doc, "/on_disk_format_version")),
    ]
}

pub async fn execute(ctx: &Context) -> Result<()> {
    let result = api::request_json_logged(ctx, Method::GET, "/v1/capabilities", None).await;
    let doc = match result {
        Ok(doc) => doc,
        Err(e) => {
            if let Some(api_err) = e.downcast_ref::<ApiHttpError>() {
                if api_err.status == 404 || api_err.status == 405 {
                    anyhow::bail!(
                        "GET /v1/capabilities not found: this server predates Dakera v0.12"
                    );
                }
            }
            return Err(e);
        }
    };
    if matches!(ctx.format, OutputFormat::Json | OutputFormat::Compact) {
        output::print_item(&doc, ctx.format);
    } else {
        output::print_kv(&summary(&doc), ctx.format);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "capabilities_version": 1,
            "server_version": "0.12.0",
            "api_versions": ["v1"],
            "default_model": "bge-large",
            "search_mode": "hnsw",
            "scoring": {"strategy": "single-vector"},
            "attachments": {
                "enabled": false,
                "max_bytes": 26214400,
                "transcription": {"model": "whisper-tiny.en"}
            },
            "vision": {"enabled": true},
            "records": {"enabled": false},
            "query_languages": ["en", "de"],
            "reembed_pending": false
        })
    }

    #[test]
    fn show_renders_strings_numbers_arrays_and_missing() {
        let doc = sample();
        assert_eq!(show(&doc, "/server_version"), "0.12.0");
        assert_eq!(show(&doc, "/attachments/max_bytes"), "26214400");
        assert_eq!(show(&doc, "/query_languages"), "en, de");
        assert_eq!(show(&doc, "/nope"), "-");
    }

    #[test]
    fn switches_name_the_variable_that_turns_a_feature_on() {
        let doc = sample();
        let off = show_switch(&doc, "/attachments/enabled", "DAKERA_ATTACHMENTS");
        assert!(off.contains("disabled"));
        assert!(off.contains("DAKERA_ATTACHMENTS"));
        let on = show_switch(&doc, "/vision/enabled", "DAKERA_VISION");
        assert_eq!(on, "enabled");
    }

    #[test]
    fn summary_covers_the_feature_rows() {
        let rows = summary(&sample());
        let labels: Vec<&str> = rows.iter().map(|(k, _)| *k).collect();
        assert!(labels.contains(&"Attachments"));
        assert!(labels.contains(&"Image indexing"));
        assert!(labels.contains(&"Records"));
        let model = rows.iter().find(|(k, _)| *k == "Default model").unwrap();
        assert_eq!(model.1, "bge-large");
    }
}
