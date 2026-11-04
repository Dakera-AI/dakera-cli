//! Forgetting memories (Dakera v0.12.4): by id, by description (`--about`),
//! by a user's request to forget (`--request`); reading messages for
//! requests to forget (`forget-detect`); and the agent's trash of restorable
//! deletions (`trash`, `restore`).
//!
//! * `forget`         `POST /v1/memory/forget`
//! * `forget-detect`  `POST /v1/memory/forget/detect`
//! * `trash`          `GET  /v1/memory/deduplicated?agent_id=`
//! * `restore`        `POST /v1/memory/deduplicated/{id}/restore?agent_id=`

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

/// The most messages one `forget-detect` request reads.
pub const MAX_DETECT_TEXTS: usize = 64;

/// The longest message (`--about`, `--request`, a `forget-detect` text), in
/// characters.
pub const MAX_TEXT_CHARS: usize = 8_000;

/// The most memories one `--about` / `--request` forget removes.
pub const MAX_FORGET_LIMIT: usize = 50;

/// The body of `POST /v1/memory/forget` from the command's arguments.
pub fn forget_body(agent_id: &str, sub: &ArgMatches) -> Result<Value> {
    let ids: Vec<String> = sub
        .get_many::<String>("memory_id")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let about = sub.get_one::<String>("about");
    let request = sub.get_one::<String>("request");
    if ids.is_empty() && about.is_none() && request.is_none() {
        return Err(input_error(
            "say what to forget: one or more memory ids, --about <description> or \
             --request <the user's message>",
        ));
    }
    for (flag, text) in [("--about", about), ("--request", request)] {
        if let Some(text) = text {
            if text.trim().is_empty() {
                return Err(input_error(format!("{flag} is empty")));
            }
            if text.chars().count() > MAX_TEXT_CHARS {
                return Err(input_error(format!(
                    "{flag} is longer than {MAX_TEXT_CHARS} characters"
                )));
            }
        }
    }
    let mut body = json!({ "agent_id": agent_id });
    if !ids.is_empty() {
        body["memory_ids"] = json!(ids);
    }
    if let Some(about) = about {
        body["about"] = json!(about);
    }
    if let Some(request) = request {
        body["request"] = json!(request);
    }
    if let Some(until) = sub.get_one::<u64>("until") {
        body["until"] = json!(until);
    }
    if let Some(score) = sub.get_one::<f64>("min-score") {
        if about.is_none() {
            return Err(input_error("--min-score goes with --about"));
        }
        if !(0.0..=1.0).contains(score) {
            return Err(input_error(format!(
                "--min-score is a relevance between 0 and 1, got {score}"
            )));
        }
        body["min_score"] = json!(score);
    }
    if let Some(limit) = sub.get_one::<usize>("limit") {
        if *limit == 0 || *limit > MAX_FORGET_LIMIT {
            return Err(input_error(format!(
                "--limit is between 1 and {MAX_FORGET_LIMIT}, got {limit}"
            )));
        }
        body["limit"] = json!(limit);
    }
    if sub.get_flag("restorable") {
        body["restorable"] = json!(true);
    }
    if sub.get_flag("dry-run") {
        body["dry_run"] = json!(true);
    }
    Ok(body)
}

/// "1 memory", "3 memories".
fn memories(n: u64) -> String {
    if n == 1 {
        "1 memory".to_string()
    } else {
        format!("{n} memories")
    }
}

/// One forgotten memory id.
#[derive(Debug, Serialize)]
struct IdRow {
    id: String,
}

/// The ids in `answer[key]`.
fn ids_of(answer: &Value, key: &str) -> Vec<String> {
    answer
        .get(key)
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// How the server read a `--request` (`intent`), for the table output.
pub fn intent_line(intent: &Value) -> Option<String> {
    let probability = intent.get("probability")?.as_f64()?;
    let threshold = intent
        .get("threshold")
        .and_then(Value::as_f64)
        .unwrap_or(f64::NAN);
    let is_request = api::bool_of(intent, "is_request").unwrap_or(false);
    let verdict = if is_request {
        "reads as a request to forget"
    } else {
        "does not read as a request to forget"
    };
    let mut line =
        format!("The message {verdict} (probability {probability:.2}, threshold {threshold:.2})");
    if let Some(sentence) = api::str_of(intent, "sentence") {
        line.push_str(&format!("; the request is the sentence \"{sentence}\""));
    }
    Some(line)
}

/// `(major, minor, patch)` of a server version string (`0.12.4`,
/// `0.12.4-rc1`, `v0.12.4`).
pub fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// The forget options an older server does not read. A v0.12.3 server
/// ignores them, and would delete the listed ids although `--dry-run` was
/// asked, or delete them for good although `--restorable` was.
fn new_forget_options(sub: &ArgMatches) -> Vec<&'static str> {
    let mut used = Vec::new();
    for (id, flag) in [("about", "--about"), ("request", "--request")] {
        if sub.get_one::<String>(id).is_some() {
            used.push(flag);
        }
    }
    for (id, flag) in [("dry-run", "--dry-run"), ("restorable", "--restorable")] {
        if sub.get_flag(id) {
            used.push(flag);
        }
    }
    used
}

/// Refuse options a server before v0.12.4 would ignore (see
/// [`new_forget_options`]). The version comes from `/health`; when it cannot
/// be read the request goes ahead.
async fn require_v0124(ctx: &Context, options: &[&str]) -> Result<()> {
    if options.is_empty() {
        return Ok(());
    }
    let Ok(probe) = api::probe(ctx, "/health").await else {
        return Ok(());
    };
    let Some(version) = api::str_of(&probe.body, "version") else {
        return Ok(());
    };
    match parse_version(version) {
        Some(v) if v < (0, 12, 4) => Err(input_error(format!(
            "{} need(s) Dakera server v0.12.4 or later; this server is {version}, which would \
             ignore {} and act on the rest of the request. Nothing was sent",
            options.join(", "),
            if options.len() == 1 { "it" } else { "them" }
        ))),
        _ => Ok(()),
    }
}

async fn forget(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent_id").unwrap();
    let body = forget_body(agent_id, sub)?;
    require_v0124(ctx, &new_forget_options(sub)).await?;
    let dry_run = sub.get_flag("dry-run");
    let restorable = sub.get_flag("restorable");
    let answer =
        api::request_json_logged(ctx, Method::POST, "/v1/memory/forget", Some(&body)).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&answer, ctx.format);
        return Ok(());
    }
    let intent = answer.get("intent");
    if let Some(line) = intent.and_then(intent_line) {
        output::info(&line);
    }
    let not_a_request = intent
        .and_then(|i| api::bool_of(i, "is_request"))
        .is_some_and(|is| !is);
    if not_a_request {
        output::warning("Nothing was forgotten.");
        return Ok(());
    }
    let ids = ids_of(&answer, "forgotten_ids");
    let deleted = api::u64_of(&answer, "deleted_count").unwrap_or(0);
    if dry_run {
        output::info(&format!(
            "[dry-run] Would forget {}; nothing was deleted",
            memories(ids.len() as u64)
        ));
    } else if deleted == 0 {
        output::info("Nothing matched; nothing was forgotten");
    } else if restorable {
        output::success(&format!(
            "Forgot {} of '{agent_id}' (restorable: `dk memory trash {agent_id}` lists them, \
             `dk memory restore {agent_id} <id>` puts one back)",
            memories(deleted)
        ));
    } else {
        output::success(&format!("Forgot {} of '{agent_id}'", memories(deleted)));
    }
    if !ids.is_empty() {
        let rows: Vec<IdRow> = ids.into_iter().map(|id| IdRow { id }).collect();
        output::print_data(&rows, ctx.format);
    }
    Ok(())
}

/// The messages of `forget-detect`: the arguments, or one per non-empty line
/// of `--file` (`-` reads stdin).
pub fn detect_texts(args: Vec<String>, file: Option<&str>) -> Result<Vec<String>> {
    let mut texts = args;
    if let Some(text) = file {
        texts.extend(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string),
        );
    }
    if texts.is_empty() {
        return Err(input_error(
            "give at least one message, as an argument or with --file",
        ));
    }
    if texts.len() > MAX_DETECT_TEXTS {
        return Err(input_error(format!(
            "at most {MAX_DETECT_TEXTS} messages per request, got {}",
            texts.len()
        )));
    }
    if let Some(i) = texts
        .iter()
        .position(|t| t.chars().count() > MAX_TEXT_CHARS)
    {
        return Err(input_error(format!(
            "message {} is longer than {MAX_TEXT_CHARS} characters",
            i + 1
        )));
    }
    Ok(texts)
}

/// One message of `forget-detect`, as a table row.
#[derive(Debug, Serialize)]
pub struct DetectRow {
    pub message: String,
    pub request: String,
    pub probability: String,
    pub sentence: String,
}

/// The rows of a `POST /v1/memory/forget/detect` answer, one per message.
pub fn detect_rows(texts: &[String], answer: &Value) -> Vec<DetectRow> {
    let empty = Vec::new();
    let results = answer
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    texts
        .iter()
        .zip(results)
        .map(|(text, r)| DetectRow {
            message: text.clone(),
            request: if api::bool_of(r, "is_request").unwrap_or(false) {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            probability: r
                .get("probability")
                .and_then(Value::as_f64)
                .map(|p| format!("{p:.3}"))
                .unwrap_or_else(|| "-".to_string()),
            sentence: api::str_of(r, "sentence").unwrap_or("-").to_string(),
        })
        .collect()
}

async fn forget_detect(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let args: Vec<String> = sub
        .get_many::<String>("text")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let file = match sub.get_one::<String>("file").map(String::as_str) {
        Some("-") => {
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
    let texts = detect_texts(args, file.as_deref())?;
    let body = json!({ "texts": texts });
    let answer =
        api::request_json_logged(ctx, Method::POST, "/v1/memory/forget/detect", Some(&body))
            .await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&answer, ctx.format);
        return Ok(());
    }
    let rows = detect_rows(&texts, &answer);
    let requests = rows.iter().filter(|r| r.request == "yes").count();
    output::info(&format!(
        "{requests} of {} message(s) read as a request to forget (model {}, threshold {})",
        rows.len(),
        api::str_of(&answer, "model").unwrap_or("?"),
        answer
            .get("threshold")
            .and_then(Value::as_f64)
            .map(|t| format!("{t:.2}"))
            .unwrap_or_else(|| "?".to_string()),
    ));
    output::print_data(&rows, ctx.format);
    if requests > 0 {
        output::info("Forget what one means: dk memory forget <agent> --request '<message>'");
    }
    Ok(())
}

/// `?agent_id=` of the trash routes.
fn agent_query(agent_id: &str) -> String {
    format!("agent_id={}", api::segment(agent_id))
}

/// One restorable memory of the agent's trash.
#[derive(Debug, Serialize)]
struct TrashRow {
    id: String,
    kind: String,
    survivor_id: String,
    deleted_at: String,
    restorable_until: String,
    content: String,
}

fn trash_rows(answer: &Value) -> Vec<TrashRow> {
    let empty = Vec::new();
    let text = |v: &Value, key: &str| api::str_of(v, key).unwrap_or("-").to_string();
    let time = |v: &Value, key: &str| {
        api::u64_of(v, key)
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".to_string())
    };
    answer
        .get("memories")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
        .iter()
        .map(|m| TrashRow {
            id: text(m, "id"),
            kind: text(m, "kind"),
            survivor_id: text(m, "survivor_id"),
            deleted_at: time(m, "deleted_at"),
            restorable_until: time(m, "restorable_until"),
            content: text(m, "content"),
        })
        .collect()
}

async fn trash(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent_id").unwrap();
    let path = format!("/v1/memory/deduplicated?{}", agent_query(agent_id));
    let answer = api::request_json_logged(ctx, Method::GET, &path, None).await?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&answer, ctx.format);
        return Ok(());
    }
    let rows = trash_rows(&answer);
    if rows.is_empty() {
        output::info(&format!("No restorable memories for '{agent_id}'"));
        return Ok(());
    }
    output::info(&format!(
        "{} restorable memories of '{agent_id}' (kind: deletion = forgotten with --restorable; \
         deduplication / consolidation = merged into survivor_id). Times are Unix seconds.",
        rows.len()
    ));
    output::print_data(&rows, ctx.format);
    Ok(())
}

async fn restore(ctx: &Context, sub: &ArgMatches) -> Result<()> {
    let agent_id = sub.get_one::<String>("agent_id").unwrap();
    let memory_id = sub.get_one::<String>("memory_id").unwrap();
    let path = format!(
        "/v1/memory/deduplicated/{}/restore?{}",
        api::segment(memory_id),
        agent_query(agent_id)
    );
    let memory = api::request_json_logged(ctx, Method::POST, &path, None).await?;
    if matches!(ctx.format, OutputFormat::Table) {
        output::success(&format!("Memory '{memory_id}' restored for '{agent_id}'"));
    }
    output::print_item(&memory, ctx.format);
    Ok(())
}

/// Run `dk memory forget | forget-detect | trash | restore`.
pub async fn execute(ctx: &Context, name: &str, sub: &ArgMatches) -> Result<()> {
    match name {
        "forget" => forget(ctx, sub).await,
        "forget-detect" => forget_detect(ctx, sub).await,
        "trash" => trash(ctx, sub).await,
        "restore" => restore(ctx, sub).await,
        other => anyhow::bail!("not a forget command: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forget_matches(args: &[&str]) -> ArgMatches {
        let m = crate::cli::build_memory_command()
            .try_get_matches_from(args)
            .expect("parse");
        m.subcommand_matches("forget").unwrap().clone()
    }

    #[test]
    fn forget_by_id_keeps_the_old_body() {
        let sub = forget_matches(&["memory", "forget", "bot", "m1"]);
        let body = forget_body("bot", &sub).unwrap();
        assert_eq!(body, json!({"agent_id": "bot", "memory_ids": ["m1"]}));
    }

    #[test]
    fn forget_by_request_sends_every_option() {
        let sub = forget_matches(&[
            "memory",
            "forget",
            "bot",
            "--request",
            "Oublie mon adresse",
            "--until",
            "1700000000",
            "--limit",
            "5",
            "--restorable",
            "--dry-run",
        ]);
        let body = forget_body("bot", &sub).unwrap();
        assert_eq!(
            body,
            json!({
                "agent_id": "bot",
                "request": "Oublie mon adresse",
                "until": 1_700_000_000u64,
                "limit": 5,
                "restorable": true,
                "dry_run": true
            })
        );
    }

    #[test]
    fn forget_by_description_takes_min_score() {
        let sub = forget_matches(&[
            "memory",
            "forget",
            "bot",
            "--about",
            "I love sushi",
            "--min-score",
            "0.7",
        ]);
        let body = forget_body("bot", &sub).unwrap();
        assert_eq!(body["about"], "I love sushi");
        assert!((body["min_score"].as_f64().unwrap() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn forget_needs_something_to_forget() {
        let sub = forget_matches(&["memory", "forget", "bot"]);
        assert!(forget_body("bot", &sub).is_err());
    }

    #[test]
    fn forget_refuses_out_of_range_options() {
        let sub = forget_matches(&["memory", "forget", "bot", "--about", "x", "--limit", "51"]);
        assert!(forget_body("bot", &sub).is_err());
        let sub = forget_matches(&[
            "memory",
            "forget",
            "bot",
            "--about",
            "x",
            "--min-score",
            "2",
        ]);
        assert!(forget_body("bot", &sub).is_err());
        let sub = forget_matches(&["memory", "forget", "bot", "--about", "  "]);
        assert!(forget_body("bot", &sub).is_err());
    }

    #[test]
    fn about_and_request_are_exclusive_and_their_options_need_one() {
        let cmd = crate::cli::build_memory_command;
        assert!(cmd()
            .try_get_matches_from(["memory", "forget", "bot", "--about", "a", "--request", "b"])
            .is_err());
        assert!(cmd()
            .try_get_matches_from(["memory", "forget", "bot", "m1", "--until", "5"])
            .is_err());
        // clap counts --request as the group --about belongs to: forget_body refuses it.
        let sub = forget_matches(&[
            "memory",
            "forget",
            "bot",
            "--request",
            "r",
            "--min-score",
            "0.5",
        ]);
        assert!(forget_body("bot", &sub).is_err());
        assert!(cmd()
            .try_get_matches_from(["memory", "forget", "bot", "m1", "--restorable", "--dry-run"])
            .is_ok());
    }

    #[test]
    fn versions_parse_and_compare() {
        assert_eq!(parse_version("0.12.4"), Some((0, 12, 4)));
        assert_eq!(parse_version("v0.12.10-rc1"), Some((0, 12, 10)));
        assert!(parse_version("0.12.3").unwrap() < (0, 12, 4));
        assert!(parse_version("0.13.0").unwrap() >= (0, 12, 4));
        assert_eq!(parse_version("dev"), None);
    }

    #[test]
    fn new_forget_options_names_what_an_older_server_ignores() {
        let sub = forget_matches(&["memory", "forget", "bot", "m1"]);
        assert!(new_forget_options(&sub).is_empty());
        let sub = forget_matches(&["memory", "forget", "bot", "m1", "--dry-run", "--restorable"]);
        assert_eq!(new_forget_options(&sub), vec!["--dry-run", "--restorable"]);
    }

    #[test]
    fn intent_line_says_how_the_request_was_read() {
        let yes = json!({"probability": 0.97, "threshold": 0.5, "is_request": true});
        assert!(intent_line(&yes).unwrap().contains("reads as a request"));
        let no = json!({"probability": 0.1, "threshold": 0.5, "is_request": false});
        assert!(intent_line(&no).unwrap().contains("does not read"));
        let sentence = json!({
            "probability": 0.9, "threshold": 0.6, "is_request": true,
            "sentence": "Please forget my address."
        });
        assert!(intent_line(&sentence)
            .unwrap()
            .contains("Please forget my address."));
    }

    #[test]
    fn detect_texts_reads_arguments_and_lines() {
        let texts = detect_texts(vec!["a".into()], Some("b\n\n  c  \n")).unwrap();
        assert_eq!(texts, vec!["a", "b", "c"]);
        assert!(detect_texts(vec![], None).is_err());
        let many: Vec<String> = (0..65).map(|i| i.to_string()).collect();
        assert!(detect_texts(many, None).is_err());
        assert!(detect_texts(vec!["x".repeat(8_001)], None).is_err());
    }

    #[test]
    fn detect_rows_pair_messages_with_results() {
        let answer = json!({
            "model": "multilingual-minilm",
            "threshold": 0.5,
            "sentence_threshold": 0.7,
            "results": [
                {"probability": 0.98, "is_request": true},
                {"probability": 0.02, "is_request": false},
                {"probability": 0.4, "is_request": true, "sentence": "Forget my job."}
            ]
        });
        let texts = vec![
            "forget it".to_string(),
            "hi".to_string(),
            "long".to_string(),
        ];
        let rows = detect_rows(&texts, &answer);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].request, "yes");
        assert_eq!(rows[0].probability, "0.980");
        assert_eq!(rows[1].request, "no");
        assert_eq!(rows[2].sentence, "Forget my job.");
    }

    #[test]
    fn trash_rows_read_the_list() {
        let answer = json!({
            "agent_id": "bot",
            "count": 1,
            "memories": [{
                "id": "m1", "kind": "deletion", "survivor_id": "m1",
                "deleted_at": 10, "restorable_until": 20, "content": "I love sushi"
            }]
        });
        let rows = trash_rows(&answer);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "deletion");
        assert_eq!(rows[0].restorable_until, "20");
        assert_eq!(rows[0].content, "I love sushi");
    }
}
