//! `dk config` command — show config and manage named server profiles.

use nu_ansi_term::Color;

use crate::config::{Config, Profile};

pub async fn execute(sub_matches: &clap::ArgMatches) -> anyhow::Result<()> {
    match sub_matches.subcommand() {
        Some(("profile", profile_matches)) => match profile_matches.subcommand() {
            Some(("add", add_matches)) => cmd_profile_add(add_matches).await,
            Some(("use", use_matches)) => cmd_profile_use(use_matches).await,
            Some(("list", _)) => cmd_profile_list().await,
            _ => {
                eprintln!("Usage: dk config profile <add|use|list>");
                Ok(())
            }
        },
        _ => cmd_show(sub_matches).await,
    }
}

async fn cmd_show(_matches: &clap::ArgMatches) -> anyhow::Result<()> {
    let config = Config::load();
    let file_cfg = Config::read_config_file()?;

    println!("Configuration:");
    println!("  Server URL:        {}", config.server_url);
    println!("  Default namespace: {}", config.default_namespace);
    println!("  Active profile:    {}", file_cfg.active_profile);
    if let Some(path) = Config::config_path() {
        println!(
            "  Config file:       {}{}",
            path.display(),
            if path.exists() { "" } else { " (not found)" }
        );
    }
    println!();
    println!("Environment overrides:");
    println!("  DAKERA_URL       - Server URL");
    println!("  DAKERA_NAMESPACE - Default namespace");
    println!();
    println!("Tip: run `dk config profile list` to see all profiles.");
    Ok(())
}

async fn cmd_profile_add(matches: &clap::ArgMatches) -> anyhow::Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    let url = matches.get_one::<String>("url").unwrap();
    let namespace = matches
        .get_one::<String>("namespace")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let profile = Profile {
        url: url.clone(),
        default_namespace: namespace.clone(),
    };

    Config::write_profile(name, profile)?;

    let green = Color::Green.bold();
    println!(
        "  {} Profile '{}' added (url={}, namespace={}).",
        green.paint("✓"),
        name,
        url,
        namespace
    );
    println!("  Run `dk config profile use {}` to activate it.", name);
    Ok(())
}

async fn cmd_profile_use(matches: &clap::ArgMatches) -> anyhow::Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    Config::use_profile(name)?;
    let green = Color::Green.bold();
    println!("  {} Switched to profile '{}'.", green.paint("✓"), name);
    Ok(())
}

async fn cmd_profile_list() -> anyhow::Result<()> {
    let file_cfg = Config::read_config_file()?;

    if file_cfg.profiles.is_empty() {
        println!(
            "No profiles configured. Run `dk config profile add <name> --url <url>` to add one."
        );
        return Ok(());
    }

    println!(
        "{:<20} {:<40} {:<20} ACTIVE",
        "NAME", "URL", "NAMESPACE"
    );
    println!("{}", "-".repeat(90));

    let mut names: Vec<&String> = file_cfg.profiles.keys().collect();
    names.sort();

    for name in names {
        let profile = &file_cfg.profiles[name];
        let active_marker = if *name == file_cfg.active_profile {
            "← active"
        } else {
            ""
        };
        println!(
            "{:<20} {:<40} {:<20} {}",
            name, profile.url, profile.default_namespace, active_marker
        );
    }

    Ok(())
}
