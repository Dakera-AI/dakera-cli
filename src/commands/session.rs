//! Session management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::context::Context as Ctx;
use crate::output;

#[derive(Debug, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub agent_id: String,
    pub started_at: u64,
    pub ended_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMemoryRow {
    pub id: String,
    pub content: String,
    pub importance: f32,
    pub score: f32,
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("start", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let metadata_str = sub_matches.get_one::<String>("metadata");

            let t = ctx.log_request("POST", &format!("/v1/{}/sessions", agent_id));
            let session = if let Some(m) = metadata_str {
                let metadata: serde_json::Value =
                    serde_json::from_str(m).context("Invalid metadata JSON")?;
                client
                    .start_session_with_metadata(agent_id, metadata)
                    .await?
            } else {
                client.start_session(agent_id).await?
            };
            ctx.log_response(t, "200 OK");

            output::success(&format!(
                "Session started (id: {}, agent: {})",
                session.id, session.agent_id
            ));

            output::print_item(&session, ctx.format);
        }

        Some(("end", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();
            let summary = sub_matches.get_one::<String>("summary").cloned();

            let t = ctx.log_request("PUT", &format!("/v1/sessions/{}/end", session_id));
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

            let query_string = if query_params.is_empty() {
                String::new()
            } else {
                format!("?{}", query_params.join("&"))
            };

            let path = format!("/v1/sessions{}", query_string);
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

            let t = ctx.log_request("GET", &format!("/v1/sessions/{}/memories", session_id));
            let response = client.session_memories(session_id).await;
            match &response {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let response = response?;

            if response.memories.is_empty() {
                output::info(&format!("No memories found for session '{}'", session_id));
            } else {
                output::info(&format!(
                    "Found {} memories in session '{}' (total: {})",
                    response.memories.len(),
                    session_id,
                    response.total_found
                ));
                let rows: Vec<SessionMemoryRow> = response
                    .memories
                    .into_iter()
                    .map(|m| SessionMemoryRow {
                        id: m.id,
                        content: m.content,
                        importance: m.importance,
                        score: m.score,
                    })
                    .collect();
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
    fn session_end_with_summary_flag() {
        let m = build_session_command()
            .try_get_matches_from(["session", "end", "sess-123", "--summary", "Good run"])
            .expect("session end with summary should parse");
        let sub = m.subcommand_matches("end").unwrap();
        assert_eq!(sub.get_one::<String>("summary").unwrap(), "Good run");
    }
}
