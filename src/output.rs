//! Output formatting utilities for CLI

use nu_ansi_term::{Color, Style};
use serde::Serialize;

use crate::OutputFormat;

/// Print success message
pub fn success(msg: &str) {
    let style = Style::new().fg(Color::Green).bold();
    println!("{} {}", style.paint("✓"), msg);
}

/// Print error message
pub fn error(msg: &str) {
    let style = Style::new().fg(Color::Red).bold();
    eprintln!("{} {}", style.paint("✗"), msg);
}

/// Print warning message
pub fn warning(msg: &str) {
    let style = Style::new().fg(Color::Yellow).bold();
    println!("{} {}", style.paint("⚠"), msg);
}

/// Print info message
pub fn info(msg: &str) {
    let style = Style::new().fg(Color::Blue).bold();
    println!("{} {}", style.paint("ℹ"), msg);
}

/// Format and print data based on output format
pub fn print_data<T: Serialize>(data: &[T], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
        }
        OutputFormat::Compact => {
            println!("{}", serde_json::to_string(data).unwrap_or_default());
        }
        OutputFormat::Table => {
            if data.is_empty() {
                println!("No data");
            } else {
                // Simple table-like output using JSON pretty print
                println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
            }
        }
    }
}

/// Format and print a single item
pub fn print_item<T: Serialize>(item: &T, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item).unwrap_or_default());
        }
        OutputFormat::Compact => {
            println!("{}", serde_json::to_string(item).unwrap_or_default());
        }
        OutputFormat::Table => {
            println!("{}", serde_json::to_string_pretty(item).unwrap_or_default());
        }
    }
}

/// Print key-value pairs
pub fn print_kv(pairs: &[(&str, String)], format: OutputFormat) {
    match format {
        OutputFormat::Json | OutputFormat::Compact => {
            let map: std::collections::HashMap<&str, &str> =
                pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
            if matches!(format, OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&map).unwrap_or_default());
            } else {
                println!("{}", serde_json::to_string(&map).unwrap_or_default());
            }
        }
        OutputFormat::Table => {
            let key_style = Style::new().fg(Color::Cyan).bold();
            for (key, value) in pairs {
                println!("{}: {}", key_style.paint(*key), value);
            }
        }
    }
}
