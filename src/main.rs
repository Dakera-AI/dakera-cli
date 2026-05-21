//! Dakera CLI - Command-line interface for Dakera AI Agent Memory Platform

mod cli;
mod commands;
mod config;
mod context;
pub mod error;
mod output;
mod retry;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::build_cli;
use crate::commands::{
    admin, agent, analytics, completion, config as config_cmd, health, index, init, keys,
    knowledge, memory, namespace, ops, session, vector,
};
use crate::config::Config;
use crate::context::Context;

/// Output format for CLI results
#[derive(Clone, Copy, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Compact,
}

impl From<&str> for OutputFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "compact" => OutputFormat::Compact,
            _ => OutputFormat::Table,
        }
    }
}

#[tokio::main]
async fn main() {
    let matches = build_cli().get_matches();

    let verbose = matches.get_flag("verbose");
    if verbose {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new("info"))
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    let format_str = matches.get_one::<String>("format").unwrap();
    let format = OutputFormat::from(format_str.as_str());

    if let Err(err) = run(matches, format, verbose).await {
        let cli_err = error::classify(&err);
        let exit_code = cli_err.exit_code();

        match format {
            OutputFormat::Json | OutputFormat::Compact => {
                let json_err = error::JsonError {
                    error: true,
                    code: cli_err.error_code(),
                    exit_code,
                    message: cli_err.to_string(),
                };
                let s = if matches!(format, OutputFormat::Json) {
                    serde_json::to_string_pretty(&json_err)
                } else {
                    serde_json::to_string(&json_err)
                };
                eprintln!("{}", s.unwrap_or_else(|_| cli_err.to_string()));
            }
            OutputFormat::Table => {
                output::error(&cli_err.to_string());
            }
        }

        std::process::exit(exit_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_from_json() {
        assert!(matches!(OutputFormat::from("json"), OutputFormat::Json));
    }

    #[test]
    fn output_format_from_compact() {
        assert!(matches!(
            OutputFormat::from("compact"),
            OutputFormat::Compact
        ));
    }

    #[test]
    fn output_format_from_table() {
        assert!(matches!(OutputFormat::from("table"), OutputFormat::Table));
    }

    #[test]
    fn output_format_unknown_defaults_to_table() {
        assert!(matches!(
            OutputFormat::from("unknown"),
            OutputFormat::Table
        ));
    }

    #[test]
    fn output_format_default_is_table() {
        assert!(matches!(OutputFormat::default(), OutputFormat::Table));
    }

    #[test]
    fn output_format_case_insensitive() {
        assert!(matches!(OutputFormat::from("JSON"), OutputFormat::Json));
        assert!(matches!(OutputFormat::from("COMPACT"), OutputFormat::Compact));
    }
}

async fn run(matches: clap::ArgMatches, format: OutputFormat, verbose: bool) -> anyhow::Result<()> {
    let config = match matches.get_one::<String>("profile") {
        Some(p) => Config::load_with_profile(p),
        None => Config::load(),
    };

    let cli_url = matches.get_one::<String>("url").unwrap();
    let url = if cli_url != "http://localhost:3000" {
        cli_url.clone()
    } else {
        config.server_url.clone()
    };

    let ctx = Context::new(url, format, verbose);

    match matches.subcommand() {
        Some(("init", _)) => init::execute().await?,
        Some(("health", sub_matches)) => {
            let detailed = sub_matches.get_flag("detailed");
            health::execute(&ctx, detailed).await?;
        }
        Some(("namespace", sub_matches)) => namespace::execute(&ctx, sub_matches).await?,
        Some(("vector", sub_matches)) => vector::execute(&ctx, sub_matches).await?,
        Some(("index", sub_matches)) => index::execute(&ctx, sub_matches).await?,
        Some(("ops", sub_matches)) => ops::execute(&ctx, sub_matches).await?,
        Some(("memory", sub_matches)) => memory::execute(&ctx, sub_matches).await?,
        Some(("session", sub_matches)) => session::execute(&ctx, sub_matches).await?,
        Some(("agent", sub_matches)) => agent::execute(&ctx, sub_matches).await?,
        Some(("knowledge", sub_matches)) => knowledge::execute(&ctx, sub_matches).await?,
        Some(("analytics", sub_matches)) => analytics::execute(&ctx, sub_matches).await?,
        Some(("admin", sub_matches)) => admin::execute(&ctx, sub_matches).await?,
        Some(("keys", sub_matches)) => keys::execute(&ctx, sub_matches).await?,
        Some(("completion", sub_matches)) => {
            let shell = sub_matches.get_one::<String>("shell").unwrap();
            let install = sub_matches.get_flag("install");
            completion::execute(shell, install)?;
        }
        Some(("config", sub_matches)) => config_cmd::execute(sub_matches).await?,
        _ => build_cli().print_help()?,
    }

    Ok(())
}
