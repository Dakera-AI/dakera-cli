//! Entity extraction commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use serde::Serialize;

use crate::context::Context as Ctx;
use crate::output;

#[derive(Debug, Serialize)]
pub struct EntityRow {
    pub entity: String,
    pub entity_type: String,
    pub confidence: String,
}

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("extract", sub)) => {
            let agent_id = sub.get_one::<String>("agent_id").unwrap();
            let text = sub.get_one::<String>("text").unwrap();
            let store = sub.get_flag("store");

            let mut body = serde_json::json!({
                "agent_id": agent_id,
                "text": text
            });
            if store {
                body.as_object_mut()
                    .unwrap()
                    .insert("store".to_string(), serde_json::Value::Bool(true));
            }

            let path = "/v1/entities/extract";
            let t = ctx.log_request("POST", path);
            let client = super::authed_client();
            let resp = client
                .post(format!("{}{}", ctx.url, path))
                .json(&body)
                .send()
                .await
                .with_context(|| "Failed to POST /v1/entities/extract")?;
            let status = resp.status();
            let text_body = resp.text().await?;
            ctx.log_response(t, &status.to_string());
            if !status.is_success() {
                anyhow::bail!("Request failed ({}): {}", status, text_body);
            }

            let data: serde_json::Value = serde_json::from_str(&text_body)
                .with_context(|| "Failed to parse response JSON")?;

            let entities = data
                .get("entities")
                .and_then(|e| e.as_array())
                .cloned()
                .unwrap_or_default();

            output::info(&format!("Extracted {} entity/entities", entities.len()));

            if entities.is_empty() {
                output::info("No entities found");
            } else {
                let rows: Vec<EntityRow> = entities
                    .iter()
                    .map(|e| EntityRow {
                        entity: e
                            .get("entity")
                            .or_else(|| e.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        entity_type: e
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        confidence: e
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .map(|c| format!("{:.3}", c))
                            .unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();
                output::print_data(&rows, ctx.format);
            }

            if store {
                if let Some(ids) = data.get("stored_ids") {
                    output::success(&format!("Entities stored: {}", ids));
                }
            }
        }

        _ => {
            output::error("Unknown entity subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_entity_command;

    #[test]
    fn entity_extract_requires_agent_and_text() {
        assert!(
            build_entity_command()
                .try_get_matches_from(["entity", "extract", "my-agent"])
                .is_err(),
            "entity extract without text should fail"
        );
    }

    #[test]
    fn entity_extract_parses_agent_and_text() {
        let m = build_entity_command()
            .try_get_matches_from(["entity", "extract", "my-agent", "Alice works at Dakera"])
            .expect("entity extract should parse");
        let sub = m.subcommand_matches("extract").unwrap();
        assert_eq!(sub.get_one::<String>("agent_id").unwrap(), "my-agent");
    }

    #[test]
    fn entity_extract_store_flag_defaults_false() {
        let m = build_entity_command()
            .try_get_matches_from(["entity", "extract", "agent", "some text"])
            .expect("entity extract should parse");
        let sub = m.subcommand_matches("extract").unwrap();
        assert!(!sub.get_flag("store"));
    }
}
