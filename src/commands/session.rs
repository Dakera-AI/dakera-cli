//! Session management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::output;
use crate::OutputFormat;

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

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("start", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let metadata_str = sub_matches.get_one::<String>("metadata");

            let session = if let Some(m) = metadata_str {
                let metadata: serde_json::Value =
                    serde_json::from_str(m).context("Invalid metadata JSON")?;
                client
                    .start_session_with_metadata(agent_id, metadata)
                    .await?
            } else {
                client.start_session(agent_id).await?
            };

            output::success(&format!(
                "Session started (id: {}, agent: {})",
                session.id, session.agent_id
            ));

            output::print_item(&session, format);
        }

        Some(("end", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();
            let summary = sub_matches.get_one::<String>("summary").cloned();

            let session = client.end_session(session_id, summary).await?;

            output::success(&format!("Session '{}' ended", session.id));
            output::print_item(&session, format);
        }

        Some(("get", sub_matches)) => {
            let session_id = sub_matches.get_one::<String>("session_id").unwrap();

            let session = client.get_session(session_id).await?;
            output::print_item(&session, format);
        }

        Some(("list", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent-id");
            let active_only = sub_matches.get_flag("active-only");
            let limit = *sub_matches.get_one::<u32>("limit").unwrap();

            // Build query parameters for the list endpoint
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

            // Use reqwest directly for query params the client doesn't support
            let list_url = format!("{}/v1/sessions{}", url, query_string);
            let response = dakera_client::reqwest::get(&list_url).await?;

            if response.status().is_success() {
                let body: serde_json::Value = response.json().await?;

                // The API returns { sessions: [...], total: N }
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
                    output::print_data(&rows, format);
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

            let response = client.session_memories(session_id).await?;

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
                output::print_data(&rows, format);
            }
        }

        _ => {
            output::error("Unknown session subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
