//! Session management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::memory::{
    SessionMemoriesOptions, SessionStartRequest, MAX_SESSION_IDLE_TIMEOUT_SECS,
};
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::context::Context as Ctx;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

#[derive(Debug, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub agent_id: String,
    pub started_at: u64,
    pub ended_at: String,
    /// `client` or `idle` (server v0.12.2); `-` while open or from older servers.
    pub ended_reason: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMemoryRow {
    pub id: String,
    pub content: String,
    pub importance: f32,
    pub score: f32,
    /// Full length of the content in characters (with `--preview` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_len: Option<usize>,
    /// Whether `content` is a cut preview (with `--preview` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("start", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let metadata_str = sub_matches.get_one::<String>("metadata");
            let idle_timeout = sub_matches.get_one::<u64>("idle-timeout").copied();

            let mut request = SessionStartRequest::new(agent_id.as_str());
            if let Some(m) = metadata_str {
                let metadata: serde_json::Value =
                    serde_json::from_str(m).context("Invalid metadata JSON")?;
                request = request.with_metadata(metadata);
            }
            if let Some(secs) = idle_timeout {
                if secs > MAX_SESSION_IDLE_TIMEOUT_SECS {
                    return Err(input_error(format!(
                        "--idle-timeout is at most 30d ({MAX_SESSION_IDLE_TIMEOUT_SECS} seconds), got {secs}"
                    )));
                }
                request = request.with_idle_timeout_secs(secs);
            }

            let t = ctx.log_request("POST", "/v1/sessions/start");
            let session = client.start_session_with(&request).await;
            ctx.log_response(t, if session.is_ok() { "200 OK" } else { "ERR" });
            let session = session?;

            output::success(&format!(
                "Session started (id: {}, agent: {})",
                session.id, session.agent_id
            ));

            output::print_item(&session, ctx.format);
        }

        Some(("touch", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();

            let t = ctx.log_request("POST", &format!("/v1/sessions/{}/touch", session_id));
            let response = client.touch_session(session_id).await;
            ctx.log_response(t, if response.is_ok() { "200 OK" } else { "ERR" });
            let response = response?;

            if !matches!(ctx.format, OutputFormat::Table) {
                output::print_item(&response, ctx.format);
                return Ok(());
            }
            if response.session_state == "ended" {
                output::warning(&format!(
                    "Session '{}' has already ended ({}); a touch does not re-open it",
                    session_id,
                    response.session.ended_reason.as_deref().unwrap_or("ended")
                ));
            } else {
                output::success(&format!("Session '{}' touched", session_id));
            }
            let opt = |v: Option<u64>| v.map(|t| t.to_string()).unwrap_or_else(|| "-".into());
            output::print_kv(
                &[
                    ("State", response.session_state.clone()),
                    ("Last activity", opt(response.session.last_activity_at)),
                    ("Idle deadline", opt(response.idle_deadline_at)),
                ],
                ctx.format,
            );
        }

        Some(("end", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();
            let summary = sub_matches.get_one::<String>("summary").cloned();

            let t = ctx.log_request("POST", &format!("/v1/sessions/{}/end", session_id));
            let response = client.end_session(session_id, summary).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            output::success(&format!("Session '{}' ended", response.session.id));
            output::print_item(&response, ctx.format);
        }

        Some(("get", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();

            let t = ctx.log_request("GET", &format!("/v1/sessions/{}", session_id));
            let session = client.get_session(session_id).await;
            match &session {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            output::print_item(&session?, ctx.format);
        }

        Some(("list", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent-id");
            let active_only = sub_matches.get_flag("active-only");
            let limit = *sub_matches.get_one::<u32>("limit").unwrap();

            let mut query_params = Vec::new();
            if let Some(aid) = agent_id {
                query_params.push(format!("agent_id={}", aid));
            }
            if active_only {
                query_params.push("active_only=true".to_string());
            }
            query_params.push(format!("limit={}", limit));

            let path = format!("/v1/sessions?{}", query_params.join("&"));
            let t = ctx.log_request("GET", &path);
            let list_url = format!("{}{}", ctx.url, path);
            let response = super::authed_client().get(&list_url).send().await?;
            let status_str = if response.status().is_success() {
                "200 OK"
            } else {
                "ERR"
            };
            ctx.log_response(t, status_str);

            if response.status().is_success() {
                let body: serde_json::Value = response.json().await?;

                let sessions = body
                    .get("sessions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let total = body.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

                if sessions.is_empty() {
                    output::info("No sessions found");
                } else {
                    output::info(&format!("Showing {} of {} sessions", sessions.len(), total));
                    let rows: Vec<SessionRow> = sessions
                        .iter()
                        .filter_map(|s| {
                            Some(SessionRow {
                                id: s.get("id")?.as_str()?.to_string(),
                                agent_id: s.get("agent_id")?.as_str()?.to_string(),
                                started_at: s.get("started_at")?.as_u64()?,
                                ended_at: s
                                    .get("ended_at")
                                    .and_then(|v| v.as_u64())
                                    .map(|t| t.to_string())
                                    .unwrap_or_else(|| "active".to_string()),
                                ended_reason: s
                                    .get("ended_reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("-")
                                    .to_string(),
                            })
                        })
                        .collect();
                    output::print_data(&rows, ctx.format);
                }
            } else {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                output::error(&format!("Failed to list sessions ({}): {}", status, text));
                std::process::exit(1);
            }
        }

        Some(("memories", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();
            let options = SessionMemoriesOptions {
                limit: sub_matches.get_one::<u32>("limit").copied(),
                offset: sub_matches.get_one::<u32>("offset").copied(),
                content_preview_chars: sub_matches.get_one::<u32>("preview").copied(),
            };

            let t = ctx.log_request("GET", &format!("/v1/sessions/{}/memories", session_id));
            let response = client.session_memories_with(session_id, &options).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.memories.is_empty() {
                output::info(&format!("No memories found for session '{}'", session_id));
            } else {
                let total = response.total.unwrap_or(response.memories.len());
                output::info(&format!(
                    "Found {} memories in session '{}' (total: {})",
                    response.memories.len(),
                    session_id,
                    total
                ));
                let truncated = response
                    .memories
                    .iter()
                    .filter(|m| m.content_truncated == Some(true))
                    .count();
                let rows: Vec<SessionMemoryRow> = response
                    .memories
                    .into_iter()
                    .map(|m| SessionMemoryRow {
                        id: m.id,
                        content: m.content,
                        importance: m.importance,
                        score: m.score,
                        content_len: m.content_len,
                        truncated: m.content_truncated,
                    })
                    .collect();
                if truncated > 0 && matches!(ctx.format, OutputFormat::Table) {
                    output::info(&format!(
                        "{truncated} memories are previews; `dk memory get` shows the full text"
                    ));
                }
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown session subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_session_command;

    #[test]
    fn session_start_requires_agent_id() {
        assert!(
            build_session_command()
                .try_get_matches_from(["session", "start"])
                .is_err(),
            "session start without agent_id should fail"
        );
    }

    #[test]
    fn session_end_requires_session_id() {
        assert!(
            build_session_command()
                .try_get_matches_from(["session", "end"])
                .is_err(),
            "session end without session_id should fail"
        );
    }

    #[test]
    fn session_list_limit_defaults_to_50() {
        let m = build_session_command()
            .try_get_matches_from(["session", "list"])
            .expect("session list should parse successfully");
        let sub = m.subcommand_matches("list").unwrap();
        assert_eq!(*sub.get_one::<u32>("limit").unwrap(), 50u32);
    }

    #[test]
    fn session_start_idle_timeout_parses_durations() {
        let m = build_session_command()
            .try_get_matches_from(["session", "start", "a", "--idle-timeout", "8h"])
            .unwrap();
        let sub = m.subcommand_matches("start").unwrap();
        assert_eq!(*sub.get_one::<u64>("idle-timeout").unwrap(), 8 * 3600);
        let m = build_session_command()
            .try_get_matches_from(["session", "start", "a", "--idle-timeout", "0"])
            .unwrap();
        let sub = m.subcommand_matches("start").unwrap();
        assert_eq!(*sub.get_one::<u64>("idle-timeout").unwrap(), 0);
    }

    #[test]
    fn session_touch_requires_session_id() {
        assert!(build_session_command()
            .try_get_matches_from(["session", "touch"])
            .is_err());
    }

    #[test]
    fn session_memories_preview_range_is_checked() {
        assert!(build_session_command()
            .try_get_matches_from(["session", "memories", "s", "--preview", "0"])
            .is_err());
        assert!(build_session_command()
            .try_get_matches_from(["session", "memories", "s", "--preview", "10001"])
            .is_err());
        assert!(build_session_command()
            .try_get_matches_from(["session", "memories", "s", "--preview", "120"])
            .is_ok());
    }

    #[test]
    fn session_end_with_summary_flag() {
        let m = build_session_command()
            .try_get_matches_from(["session", "end", "sess-123", "--summary", "Good run"])
            .expect("session end with summary should parse");
        let sub = m.subcommand_matches("end").unwrap();
        assert_eq!(sub.get_one::<String>("summary").unwrap(), "Good run");
    }
}
