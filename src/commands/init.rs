//! `dk init` — interactive onboarding wizard

use std::io::{self, Write};

use anyhow::Result;
use dakera_client::DakeraClient;
use nu_ansi_term::{Color, Style};

use crate::config::{Config, Profile};
use crate::output;

pub async fn execute() -> Result<()> {
    let cyan = Style::new().fg(Color::Cyan).bold();
    let green = Style::new().fg(Color::Green).bold();
    let yellow = Style::new().fg(Color::Yellow);
    let dim = Style::new().dimmed();

    println!("{}", cyan.paint("Welcome to Dakera!"));
    println!(
        "{}",
        dim.paint("This wizard sets up your local configuration.")
    );
    println!();

    // ── Step 1: Server URL ─────────────────────────────────────────────────
    let url = prompt_default("Server URL", "http://localhost:3000")?;

    // ── Step 2: Test connectivity ──────────────────────────────────────────
    print!("  Testing connection to {}... ", &url);
    io::stdout().flush()?;

    let client = DakeraClient::new(&url)?;
    let connected = match client.health().await {
        Ok(h) if h.healthy => {
            println!("{}", green.paint("OK"));
            if let Some(v) = &h.version {
                println!("  Server version: {}", v);
            }
            true
        }
        Ok(_) => {
            println!("{}", yellow.paint("server responded but reports unhealthy"));
            true
        }
        Err(e) => {
            println!();
            output::error(&format!("Cannot reach server: {}", e));
            println!();
            let cont = prompt_default("Save config and continue anyway?", "yes")?;
            if cont.trim().to_lowercase().starts_with('n') {
                println!("Aborted. Run `dk init` again when the server is reachable.");
                return Ok(());
            }
            false
        }
    };

    println!();

    // ── Step 3: Default namespace ──────────────────────────────────────────
    let ns = prompt_default("Default namespace", "default")?;

    if !ns
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        output::error("Namespace name may only contain letters, digits, hyphens, and underscores.");
        return Ok(());
    }

    // ── Step 4: Verify namespace exists (informational, non-blocking) ──────
    if connected {
        match client.list_namespaces().await {
            Ok(names) if names.contains(&ns) => {
                println!(
                    "  {} Namespace '{}' exists on server.",
                    green.paint("✓"),
                    ns
                );
            }
            Ok(_) => {
                println!(
                    "  {} Namespace '{}' not found yet — it will be created on first vector upsert.",
                    Style::new().fg(Color::Blue).paint("i"),
                    ns
                );
            }
            Err(_) => {} // non-fatal, skip
        }
    }

    println!();

    // ── Step 5: Write config ───────────────────────────────────────────────
    Config::write_profile(
        "default",
        Profile {
            url: url.clone(),
            default_namespace: ns.clone(),
        },
    )?;

    let config_path = Config::config_path().unwrap();
    println!(
        "{} Configuration saved to {}",
        green.paint("✓"),
        config_path.display()
    );
    println!();

    // ── Step 6: Quickstart snippet ─────────────────────────────────────────
    println!("{}", cyan.paint("Quick start:"));
    println!();
    println!("  # Store a memory");
    println!(
        "  dk memory store my-agent 'First memory' --namespace {}",
        ns
    );
    println!();
    println!("  # Recall memories");
    println!(
        "  dk memory recall my-agent 'query text' --namespace {} --top-k 5",
        ns
    );
    println!();
    println!("  # Check server health");
    println!("  dk health --detailed");
    println!();
    println!("  Full docs: https://dakera.ai/docs");
    println!();
    println!(
        "{}",
        green.paint("All done! Run `dk --help` to explore all commands.")
    );

    Ok(())
}

fn prompt_default(label: &str, default: &str) -> Result<String> {
    let dim = Style::new().dimmed();
    print!("  {} {}: ", label, dim.paint(format!("[{}]", default)));
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();

    Ok(if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::build_cli;

    #[test]
    fn init_subcommand_is_recognized_by_cli() {
        build_cli()
            .try_get_matches_from(["dk", "init"])
            .expect("dk init should parse successfully");
    }

    #[test]
    fn init_has_no_required_args() {
        // init is fully interactive — no required CLI args
        let m = build_cli()
            .try_get_matches_from(["dk", "init"])
            .expect("dk init requires no arguments");
        assert!(m.subcommand_matches("init").is_some());
    }

    #[test]
    fn init_does_not_accept_unknown_flags() {
        assert!(
            build_cli()
                .try_get_matches_from(["dk", "init", "--unknown-flag"])
                .is_err(),
            "init should reject unknown flags"
        );
    }
}
