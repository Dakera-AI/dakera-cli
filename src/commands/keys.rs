//! API key management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::reqwest;
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::output;

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

pub async fn execute(ctx: &Ctx, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("create", sub)) => {
            let name = sub.get_one::<String>("name").unwrap();
            let permissions = sub.get_one::<String>("permissions");
            let expires = sub.get_one::<u64>("expires");

            let mut body = serde_json::json!({ "name": name });
            body.as_object_mut().unwrap().insert(
                "scope".to_string(),
                Value::String(permissions.cloned().unwrap_or_else(|| "read".to_string())),
            );
            if let Some(exp) = expires {
                body.as_object_mut()
                    .unwrap()
                    .insert("expires_in_days".to_string(), Value::Number((*exp).into()));
            }

            let path = "/admin/keys";
            let t = ctx.log_request("POST", path);
            let result = keys_post(&ctx.url, path, Some(&body)).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

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
                output::print_item(&result, ctx.format);
            }
        }

        Some(("list", _sub)) => {
            let path = "/admin/keys";
            let t = ctx.log_request("GET", path);
            let result = keys_get(&ctx.url, path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info("API Keys");
            output::print_item(&result?, ctx.format);
        }

        Some(("get", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let path = format!("/admin/keys/{}", key_id);
            let t = ctx.log_request("GET", &path);
            let result = keys_get(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::print_item(&result?, ctx.format);
        }

        Some(("delete", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let path = format!("/admin/keys/{}", key_id);
            let t = ctx.log_request("DELETE", &path);
            let result = keys_delete(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("API key '{}' deleted", key_id));
            output::print_item(&result?, ctx.format);
        }

        Some(("deactivate", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let path = format!("/admin/keys/{}/deactivate", key_id);
            let t = ctx.log_request("POST", &path);
            let result = keys_post(&ctx.url, &path, None).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::success(&format!("API key '{}' deactivated", key_id));
            output::print_item(&result?, ctx.format);
        }

        Some(("rotate", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let path = format!("/admin/keys/{}/rotate", key_id);
            let t = ctx.log_request("POST", &path);
            let result = keys_post(&ctx.url, &path, None).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

            if let Some(new_key) = result.get("key").and_then(|k| k.as_str()) {
                output::success(&format!("API key '{}' rotated", key_id));
                output::warning("Save the new key now - it will not be shown again!");
                println!();
                println!("  New Key: {}", new_key);
                println!();
            } else {
                output::success(&format!("API key '{}' rotated", key_id));
                output::print_item(&result, ctx.format);
            }
        }

        Some(("usage", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let path = format!("/admin/keys/{}/usage", key_id);
            let t = ctx.log_request("GET", &path);
            let result = keys_get(&ctx.url, &path).await;
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            output::info(&format!("Usage for key '{}'", key_id));
            output::print_item(&result?, ctx.format);
        }

        _ => {
            output::error("Unknown keys subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
