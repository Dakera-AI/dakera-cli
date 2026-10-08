//! API key management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use dakera_client::{DakeraClient, KeyInfo, UpdateKeyRequest, WhoamiResponse};
use serde_json::Value;

use crate::context::Context as Ctx;
use crate::error::input_error;
use crate::output;
use crate::OutputFormat;

/// A namespace grant list as one cell: `all` for `None` (every namespace),
/// `none` for `[]`.
fn grants_cell(namespaces: Option<&Vec<String>>) -> String {
    match namespaces {
        None => "all".to_string(),
        Some(list) if list.is_empty() => "none".to_string(),
        Some(list) => list.join(", "),
    }
}

/// The `UpdateKeyRequest` the `keys edit` flags describe.
pub(crate) fn update_request(sub: &ArgMatches) -> Result<UpdateKeyRequest> {
    let mut req = UpdateKeyRequest::new();
    if let Some(name) = sub.get_one::<String>("name") {
        req = req.with_name(name.clone());
    }
    if sub.get_flag("all-namespaces") {
        req = req.with_all_namespaces();
    } else if sub.get_flag("no-namespaces") {
        req = req.with_namespaces(Vec::new());
    } else if let Some(list) = sub.get_many::<String>("namespaces") {
        let list: Vec<String> = list
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if list.is_empty() {
            return Err(input_error(
                "--namespaces needs at least one name (use --no-namespaces to grant none)",
            ));
        }
        req = req.with_namespaces(list);
    }
    if req.is_empty() {
        return Err(input_error(
            "nothing to change: pass --name, --namespaces, --all-namespaces or --no-namespaces",
        ));
    }
    Ok(req)
}

/// Print a key (table: key/value pairs; json: the server's fields).
fn print_key(key: &KeyInfo, format: OutputFormat) {
    if !matches!(format, OutputFormat::Table) {
        output::print_item(key, format);
        return;
    }
    let mut pairs = vec![
        ("Key ID", key.key_id.clone()),
        ("Name", key.name.clone()),
        ("Scope", key.scope.clone()),
        ("Namespaces", grants_cell(key.namespaces.as_ref())),
        ("Active", key.active.to_string()),
        (
            "Expires at",
            key.expires_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ),
    ];
    if let Some(v) = key.grants_version {
        pairs.push(("Grants version", v.to_string()));
    }
    if !key.inert_namespaces.is_empty() {
        pairs.push(("Inert grants", key.inert_namespaces.join(", ")));
    }
    output::print_kv(&pairs, format);
}

/// `dk whoami` — `GET /v1/auth/whoami` (server v0.12.2).
pub async fn whoami(ctx: &Ctx) -> Result<()> {
    let client = DakeraClient::new(&ctx.url)?;
    let t = ctx.log_request("GET", "/v1/auth/whoami");
    let who = client.whoami().await;
    ctx.log_response(t, if who.is_ok() { "200 OK" } else { "ERR" });
    let who: WhoamiResponse = who?;
    if !matches!(ctx.format, OutputFormat::Table) {
        output::print_item(&who, ctx.format);
        return Ok(());
    }
    if !who.auth_enabled {
        output::warning("Authentication is off on this server: every caller is super_admin");
    }
    let mut pairs = vec![
        ("Key ID", who.key_id.clone()),
        ("Name", who.name.clone()),
        ("Scope", who.scope.clone()),
        ("Namespaces", grants_cell(who.namespaces.as_ref())),
        ("Unrestricted", who.unrestricted.to_string()),
        (
            "Expires at",
            who.expires_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ),
        ("Grants version", who.grants_version.to_string()),
    ];
    if !who.inert_namespaces.is_empty() {
        pairs.push(("Inert grants", who.inert_namespaces.join(", ")));
    }
    output::print_kv(&pairs, ctx.format);
    if who.grants_version == 0 && who.inert_namespaces.iter().any(|g| g.ends_with('*')) {
        output::info(
            "This key predates prefix patterns: its `*` grants match nothing until its namespaces are saved again (dk keys edit --namespaces ...)",
        );
    }
    Ok(())
}

async fn keys_get(url: &str, path: &str) -> Result<Value> {
    let client = super::authed_client();
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
    let client = super::authed_client();
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
    let client = super::authed_client();
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
            let grace = sub.get_one::<u64>("grace").copied();
            if let Some(g) = grace {
                if g > dakera_client::MAX_ROTATION_GRACE_SECS {
                    return Err(input_error(format!(
                        "--grace is at most 7d ({} seconds), got {g}",
                        dakera_client::MAX_ROTATION_GRACE_SECS
                    )));
                }
            }
            let client = DakeraClient::new(&ctx.url)?;
            let path = format!("/admin/keys/{}/rotate", key_id);
            let t = ctx.log_request("POST", &path);
            let result = match grace {
                Some(g) => client.rotate_key_with_grace(key_id, g).await,
                None => client.rotate_key(key_id).await,
            };
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let result = result?;

            if !matches!(ctx.format, OutputFormat::Table) {
                output::print_item(&result, ctx.format);
                return Ok(());
            }
            output::success(&format!("API key '{}' rotated", key_id));
            output::warning("Save the new key now - it will not be shown again!");
            println!();
            println!("  New Key: {}", result.new_key);
            println!("  New Key ID: {}", result.key_id);
            match (grace, result.old_key_id.as_deref(), result.old_key_expires_at) {
                (_, Some(old), Some(at)) => {
                    println!("  Old key {old} keeps working until {at} (Unix seconds)")
                }
                (Some(g), None, _) if g > 0 => output::warning(
                    "The server did not report a grace period (it predates v0.12.2): the old key was deactivated at once",
                ),
                (_, Some(old), None) => println!("  Old key {old} was deactivated"),
                _ => {}
            }
            println!();
        }

        Some(("edit", sub)) => {
            let key_id = sub.get_one::<String>("key_id").unwrap();
            let request = update_request(sub)?;
            let client = DakeraClient::new(&ctx.url)?;
            let namespace = sub.get_one::<String>("namespace");
            let path = match namespace {
                Some(ns) => format!("/v1/namespaces/{ns}/keys/{key_id}"),
                None => format!("/admin/keys/{key_id}"),
            };
            let t = ctx.log_request("PATCH", &path);
            let result = match namespace {
                Some(ns) => client.update_namespace_key(ns, key_id, &request).await,
                None => client.update_key(key_id, &request).await,
            };
            ctx.log_response(t, if result.is_ok() { "200 OK" } else { "ERR" });
            let key = result?;
            if matches!(ctx.format, OutputFormat::Table) {
                output::success(&format!("API key '{}' updated", key_id));
            }
            print_key(&key, ctx.format);
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

#[cfg(test)]
mod tests {
    use crate::cli::build_keys_command;

    #[test]
    fn keys_create_requires_name() {
        assert!(
            build_keys_command()
                .try_get_matches_from(["keys", "create"])
                .is_err(),
            "keys create without name should fail"
        );
    }

    #[test]
    fn keys_create_with_permissions_flag() {
        let m = build_keys_command()
            .try_get_matches_from(["keys", "create", "my-key", "--permissions", "write"])
            .expect("keys create with --permissions should parse");
        let sub = m.subcommand_matches("create").unwrap();
        assert_eq!(sub.get_one::<String>("permissions").unwrap(), "write");
    }

    #[test]
    fn keys_delete_requires_key_id() {
        assert!(
            build_keys_command()
                .try_get_matches_from(["keys", "delete"])
                .is_err(),
            "keys delete without key_id should fail"
        );
    }

    #[test]
    fn keys_rotate_grace_accepts_durations() {
        let m = build_keys_command()
            .try_get_matches_from(["keys", "rotate", "dk_key_1", "--grace", "1h"])
            .unwrap();
        let sub = m.subcommand_matches("rotate").unwrap();
        assert_eq!(*sub.get_one::<u64>("grace").unwrap(), 3600);
        assert!(build_keys_command()
            .try_get_matches_from(["keys", "rotate", "dk_key_1", "--grace", "soon"])
            .is_err());
    }

    fn edit(args: &[&str]) -> Result<dakera_client::UpdateKeyRequest, String> {
        let mut argv = vec!["keys", "edit", "dk_key_1"];
        argv.extend_from_slice(args);
        let m = build_keys_command()
            .try_get_matches_from(argv)
            .map_err(|e| e.to_string())?;
        super::update_request(m.subcommand_matches("edit").unwrap()).map_err(|e| e.to_string())
    }

    #[test]
    fn keys_edit_builds_absent_null_and_list() {
        let r = edit(&["--name", "ci"]).unwrap();
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"name":"ci"}"#);
        let r = edit(&["--all-namespaces"]).unwrap();
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"namespaces":null}"#);
        let r = edit(&["--no-namespaces"]).unwrap();
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"namespaces":[]}"#);
        let r = edit(&["--namespaces", "team-*,docs"]).unwrap();
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"namespaces":["team-*","docs"]}"#
        );
    }

    #[test]
    fn keys_edit_refuses_nothing_and_conflicts() {
        assert!(edit(&[]).is_err());
        assert!(edit(&["--all-namespaces", "--namespaces", "a"]).is_err());
        assert!(edit(&["--all-namespaces", "--no-namespaces"]).is_err());
        assert!(build_keys_command()
            .try_get_matches_from(["keys", "patch", "dk_key_1", "--name", "x"])
            .is_ok());
    }

    #[test]
    fn keys_create_with_expiry_in_days() {
        let m = build_keys_command()
            .try_get_matches_from(["keys", "create", "expiring-key", "--expires", "30"])
            .expect("keys create with --expires should parse");
        let sub = m.subcommand_matches("create").unwrap();
        assert_eq!(*sub.get_one::<u64>("expires").unwrap(), 30u64);
    }
}
