//! Agent management commands

use anyhow::Result;
use clap::ArgMatches;
use dakera_client::DakeraClient;
use serde::Serialize;

use crate::context::Context;
use crate::output;

#[derive(Debug, Serialize)]
pub struct AgentRow {
    pub agent_id: String,
    pub memory_count: i64,
    pub session_count: i64,
    pub active_sessions: i64,
}

#[derive(Debug, Serialize)]
pub struct AgentMemoryRow {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: f32,
}

#[derive(Debug, Serialize)]
pub struct AgentSessionRow {
    pub id: String,
    pub started_at: u64,
    pub ended_at: String,
}

pub async fn execute(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;

    match matches.subcommand() {
        Some(("list", _)) => {
            let t = ctx.log_request("GET", "/v1/agents");
            let agents = client.list_agents().await;
            match &agents {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let agents = agents?;

            if agents.is_empty() {
                output::info("No agents found");
            } else {
                let rows: Vec<AgentRow> = agents
                    .into_iter()
                    .map(|a| AgentRow {
                        agent_id: a.agent_id,
                        memory_count: a.memory_count,
                        session_count: a.session_count,
                        active_sessions: a.active_sessions,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("memories", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let memory_type = sub_matches.get_one::<String>("type").map(|s| s.as_str());
            let limit = sub_matches.get_one::<u32>("limit").copied();

            let t = ctx.log_request("GET", &format!("/v1/agents/{}/memories", agent_id));
            let memories = client.agent_memories(agent_id, memory_type, limit).await;
            match &memories {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let memories = memories?;

            if memories.is_empty() {
                output::info(&format!("No memories found for agent '{}'", agent_id));
            } else {
                output::info(&format!(
                    "Found {} memories for agent '{}'",
                    memories.len(),
                    agent_id
                ));
                let rows: Vec<AgentMemoryRow> = memories
                    .into_iter()
                    .map(|m| AgentMemoryRow {
                        id: m.id,
                        content: m.content,
                        memory_type: format!("{:?}", m.memory_type),
                        importance: m.importance,
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        Some(("stats", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();

            let t = ctx.log_request("GET", &format!("/v1/agents/{}/stats", agent_id));
            let stats = client.agent_stats(agent_id).await;
            match &stats {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let stats = stats?;

            let pairs = [
                ("Agent ID", stats.agent_id),
                ("Total Memories", stats.total_memories.to_string()),
                ("Total Sessions", stats.total_sessions.to_string()),
                ("Active Sessions", stats.active_sessions.to_string()),
                (
                    "Avg Importance",
                    stats
                        .avg_importance
                        .map(|v| format!("{:.3}", v))
                        .unwrap_or_else(|| "-".to_string()),
                ),
                (
                    "Oldest Memory",
                    stats.oldest_memory_at.unwrap_or_else(|| "-".to_string()),
                ),
                (
                    "Newest Memory",
                    stats.newest_memory_at.unwrap_or_else(|| "-".to_string()),
                ),
            ];

            output::print_kv(
                &pairs
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>(),
                ctx.format,
            );

            if !stats.memories_by_type.is_empty() {
                println!();
                output::info("Memories by type:");
                for (mem_type, count) in &stats.memories_by_type {
                    println!("  {}: {}", mem_type, count);
                }
            }
        }

        Some(("sessions", sub_matches)) => {
            let agent_id = sub_matches.get_one::<String>("agent_id").unwrap();
            let active_only = sub_matches.get_flag("active-only");
            let limit = sub_matches.get_one::<u32>("limit").copied();

            let active_filter = if active_only { Some(true) } else { None };
            let t = ctx.log_request("GET", &format!("/v1/agents/{}/sessions", agent_id));
            let sessions = client.agent_sessions(agent_id, active_filter, limit).await;
            match &sessions {
                Ok(_) => ctx.log_response(t, "200 OK"),
                Err(_) => ctx.log_response(t, "ERR"),
            }
            let sessions = sessions?;

            if sessions.is_empty() {
                output::info(&format!("No sessions found for agent '{}'", agent_id));
            } else {
                output::info(&format!(
                    "Found {} sessions for agent '{}'",
                    sessions.len(),
                    agent_id
                ));
                let rows: Vec<AgentSessionRow> = sessions
                    .into_iter()
                    .map(|s| AgentSessionRow {
                        id: s.id,
                        started_at: s.started_at,
                        ended_at: s
                            .ended_at
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "active".to_string()),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }
        }

        _ => {
            output::error("Unknown agent subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
