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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputFormat;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestRow {
        id: u32,
        name: String,
    }

    #[test]
    fn test_print_data_json_no_panic() {
        let data = vec![
            TestRow {
                id: 1,
                name: "alice".into(),
            },
            TestRow {
                id: 2,
                name: "bob".into(),
            },
        ];
        print_data(&data, OutputFormat::Json);
    }

    #[test]
    fn test_print_data_compact_no_panic() {
        let data = vec![TestRow {
            id: 1,
            name: "test".into(),
        }];
        print_data(&data, OutputFormat::Compact);
    }

    #[test]
    fn test_print_data_table_no_panic() {
        let data = vec![TestRow {
            id: 1,
            name: "test".into(),
        }];
        print_data(&data, OutputFormat::Table);
    }

    #[test]
    fn test_print_data_empty_table_no_panic() {
        let data: Vec<TestRow> = vec![];
        print_data(&data, OutputFormat::Table);
    }

    #[test]
    fn test_print_item_all_formats_no_panic() {
        let item = TestRow {
            id: 42,
            name: "example".into(),
        };
        print_item(&item, OutputFormat::Json);
        print_item(&item, OutputFormat::Compact);
        print_item(&item, OutputFormat::Table);
    }

    #[test]
    fn test_print_kv_all_formats_no_panic() {
        let pairs = vec![
            ("url", "http://localhost:3000".to_string()),
            ("ns", "default".to_string()),
        ];
        print_kv(&pairs, OutputFormat::Json);
        print_kv(&pairs, OutputFormat::Compact);
        print_kv(&pairs, OutputFormat::Table);
    }

    #[test]
    fn test_json_compact_differ_in_whitespace() {
        let data = vec![TestRow {
            id: 1,
            name: "x".into(),
        }];
        let pretty = serde_json::to_string_pretty(&data).unwrap();
        let compact = serde_json::to_string(&data).unwrap();
        assert!(
            pretty.len() > compact.len(),
            "Pretty JSON should be longer than compact"
        );
    }

    #[test]
    fn test_kv_json_serialization_roundtrip() {
        let pairs = vec![("key", "value".to_string())];
        let map: std::collections::HashMap<&str, &str> =
            pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let json_str = serde_json::to_string_pretty(&map).unwrap();
        assert!(json_str.contains("\"key\""));
        assert!(json_str.contains("\"value\""));
    }

    #[test]
    fn test_print_data_json_output_is_valid_json() {
        let data = vec![TestRow {
            id: 1,
            name: "test".into(),
        }];
        let s = serde_json::to_string_pretty(&data).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(parsed.is_array());
    }
}
