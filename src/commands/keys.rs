//! API key management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde_json::Value;

use crate::output;
use crate::OutputFormat;

/// Helper: make a GET request, returning JSON Value
async fn keys_get(url: &str, path: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}{}", url, path))
        .send()
        .await
        .with_context(|| format!("Failed to GET {}", path))?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, body);
    }
    serde_json::from_str(&body).with_context(|| "Failed to parse response JSON")
}

/// Helper: make a POST request with optional JSON body, returning JSON Value
async fn keys_post(url: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let client = reqwest::Client::new();
    let mut req = client.post(format!("{}{}", url, path));
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("Failed to POST {}", path))?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    if text.is_empty() {
        Ok(Value::Object(serde_json::Map::new()))
    } else {
        serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
    }
}

/// Helper: make a DELETE request, returning JSON Value
async fn keys_delete(url: &str, path: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}{}", url, path))
        .send()
        .await
        .with_context(|| format!("Failed to DELETE {}", path))?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Request failed ({}): {}", status, text);
    }
    if text.is_empty() {
        Ok(Value::Object(serde_json::Map::new()))
    } else {
        serde_json::from_str(&text).with_context(|| "Failed to parse response JSON")
    }
}

fn print_value(value: &Value, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
        }
        OutputFormat::Compact => {
            println!("{}", serde_json::to_string(value).unwrap_or_default());
        }
        OutputFormat::Table => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
        }
    }
}

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    match matches.subcommand() {
        Some(("create", sub)) => {
            let name = sub.get_one::<String>("name").unwrap();
            let permissions = sub.get_one::<String>("permissions");
            let expires = sub.get_one::<u64>("expires");

            let mut body = serde_json::json!({
                "name": name,
            });
            if let Some(perms) = permissions {
                // scope is what the API expects
                body.as_object_mut()
                    .unwrap()
                    .insert("scope".to_string(), Value::String(perms.clone()));
            } else {
                body.as_object_mut()
                    .unwrap()
                    .insert("scope".to_string(), Value::String("read".to_string()));
            }
            if let Some(exp) = expires {
                body.as_object_mut()
                    .unwrap()
                    .insert("expires_in_days".to_string(), Value::Number((*exp).into()));
            }

            let result = keys_post(url, "/admin/keys", Some(&body)).await?;

            // The key is only shown once, highlight it
            if let Some(key) = result.get("key").and_then(|k| k.as_str()) {
                output::success(&format!("API key created: {}", name));
                output::warning("Save this key now - it will not be shown again!");
                println!();
                println!("  Key: {}", key);
                if let Some(id) = result.get("key_id").and_then(|k| k.as_str()) {
                    println!("  Key ID: {}", id);
                }
                println!();
            } else {
                output::success(&format!("API key '{}' created", name));
                print_value(&result, format);
            }
        }

        Some(("list", _sub)) => {
            let result = keys_get(url, "/admin/keys").await?;
            output::info("API Keys");
            print_value(&result, format);
        }

        Some(("get", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let result = keys_get(url, &format!("/admin/keys/{}", key_id)).await?;
            print_value(&result, format);
        }

        Some(("delete", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let result = keys_delete(url, &format!("/admin/keys/{}", key_id)).await?;
            output::success(&format!("API key '{}' deleted", key_id));
            print_value(&result, format);
        }

        Some(("deactivate", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let result =
                keys_post(url, &format!("/admin/keys/{}/deactivate", key_id), None).await?;
            output::success(&format!("API key '{}' deactivated", key_id));
            print_value(&result, format);
        }

        Some(("rotate", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let result = keys_post(url, &format!("/admin/keys/{}/rotate", key_id), None).await?;

            // Rotation returns a new key - highlight it
            if let Some(new_key) = result.get("key").and_then(|k| k.as_str()) {
                output::success(&format!("API key '{}' rotated", key_id));
                output::warning("Save the new key now - it will not be shown again!");
                println!();
                println!("  New Key: {}", new_key);
                println!();
            } else {
                output::success(&format!("API key '{}' rotated", key_id));
                print_value(&result, format);
            }
        }

        Some(("usage", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let result = keys_get(url, &format!("/admin/keys/{}/usage", key_id)).await?;
            output::info(&format!("Usage for key '{}'", key_id));
            print_value(&result, format);
        }

        _ => {
            output::error("Unknown keys subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
