//! Vector management commands

use anyhow::{Context, Result};
use clap::ArgMatches;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use dakera_client::{AggregationRequest, ColumnUpsertRequest, DeleteRequest, ExportRequest, MultiVectorSearchRequest, QueryExplainRequest, QueryRequest, UnifiedQueryRequest, UpsertRequest, Vector, DakeraClient};

use crate::output;
use crate::OutputFormat;

#[derive(Debug, Serialize)]
pub struct QueryResultRow {
    pub id: String,
    pub score: f32,
    pub metadata: Option<serde_json::Value>,
}

pub async fn execute(url: &str, matches: &ArgMatches, format: OutputFormat) -> Result<()> {
    let client = DakeraClient::new(url)?;

    match matches.subcommand() {
        Some(("upsert", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();
            let batch_size = *sub_matches.get_one::<usize>("batch-size").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let vectors: Vec<Vector> = serde_json::from_str(&content)
                .with_context(|| "Failed to parse JSON. Expected array of vectors")?;

            let total = vectors.len();
            output::info(&format!("Upserting {} vectors to namespace '{}'", total, namespace));

            let mut upserted = 0;
            for chunk in vectors.chunks(batch_size) {
                let request = UpsertRequest {
                    vectors: chunk.to_vec(),
                };
                client.upsert(namespace, request).await?;
                upserted += chunk.len();
                println!("  Progress: {}/{} ({:.1}%)", upserted, total, (upserted as f64 / total as f64) * 100.0);
            }

            output::success(&format!("Successfully upserted {} vectors", total));
        }

        Some(("upsert-one", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let id = sub_matches.get_one::<String>("id").unwrap();
            let values: Vec<f32> = sub_matches.get_many::<f32>("values")
                .unwrap()
                .copied()
                .collect();
            let metadata_str = sub_matches.get_one::<String>("metadata");

            let metadata = if let Some(m) = metadata_str {
                Some(serde_json::from_str(m).context("Invalid metadata JSON")?)
            } else {
                None
            };

            let vector = Vector {
                id: id.clone(),
                values,
                metadata,
            };

            client.upsert_one(namespace, vector).await?;
            output::success(&format!("Successfully upserted vector '{}'", id));
        }

        Some(("query", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let values: Vec<f32> = sub_matches.get_many::<f32>("values")
                .unwrap()
                .copied()
                .collect();
            let top_k = *sub_matches.get_one::<u32>("top-k").unwrap();
            let include_metadata = sub_matches.get_flag("include-metadata");
            let filter_str = sub_matches.get_one::<String>("filter");

            let filter = if let Some(f) = filter_str {
                Some(serde_json::from_str(f).context("Invalid filter JSON")?)
            } else {
                None
            };

            let mut request = QueryRequest::new(values, top_k)
                .include_metadata(include_metadata);
            if let Some(f) = filter {
                request = request.with_filter(f);
            }

            let response = client.query(namespace, request).await?;

            if response.matches.is_empty() {
                output::info("No matches found");
            } else {
                let rows: Vec<QueryResultRow> = response
                    .matches
                    .into_iter()
                    .map(|m| QueryResultRow {
                        id: m.id,
                        score: m.score,
                        metadata: m.metadata.map(|h| serde_json::Value::Object(h.into_iter().collect())),
                    })
                    .collect();
                output::print_data(&rows, format);
            }
        }

        Some(("query-file", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let request: QueryRequest =
                serde_json::from_str(&content).context("Failed to parse query JSON")?;

            let response = client.query(namespace, request).await?;

            if response.matches.is_empty() {
                output::info("No matches found");
            } else {
                let rows: Vec<QueryResultRow> = response
                    .matches
                    .into_iter()
                    .map(|m| QueryResultRow {
                        id: m.id,
                        score: m.score,
                        metadata: m.metadata.map(|h| serde_json::Value::Object(h.into_iter().collect())),
                    })
                    .collect();
                output::print_data(&rows, format);
            }
        }

        Some(("delete", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let ids: Vec<String> = sub_matches.get_many::<String>("ids")
                .map(|v| v.cloned().collect())
                .unwrap_or_default();
            let all = sub_matches.get_flag("all");
            let yes = sub_matches.get_flag("yes");

            if all {
                if !yes {
                    output::warning(&format!(
                        "This will delete ALL vectors in namespace '{}'",
                        namespace
                    ));
                    print!("Are you sure? [y/N]: ");
                    use std::io::{self, Write};
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if !input.trim().eq_ignore_ascii_case("y") {
                        output::info("Deletion cancelled");
                        return Ok(());
                    }
                }
                output::warning("Bulk deletion not yet implemented");
            } else if ids.is_empty() {
                output::error("No vector IDs specified. Use --ids or --all");
                std::process::exit(1);
            } else {
                let request = DeleteRequest { ids };
                let response = client.delete(namespace, request).await?;
                output::success(&format!("Deleted {} vectors", response.deleted_count));
            }
        }

        Some(("multi-search", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let request: MultiVectorSearchRequest = serde_json::from_str(&content)
                .context("Failed to parse multi-vector search JSON")?;

            output::info(&format!("Running multi-vector search on namespace '{}'", namespace));
            let response = client.multi_vector_search(namespace, request).await?;
            let json = serde_json::to_value(&response).context("Failed to serialize response")?;
            output::print_item(&json, format);
        }

        Some(("unified-query", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let request: UnifiedQueryRequest = serde_json::from_str(&content)
                .context("Failed to parse unified query JSON")?;

            output::info(&format!("Running unified query on namespace '{}'", namespace));
            let response = client.unified_query(namespace, request).await?;
            let json = serde_json::to_value(&response).context("Failed to serialize response")?;
            output::print_item(&json, format);
        }

        Some(("aggregate", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let request: AggregationRequest = serde_json::from_str(&content)
                .context("Failed to parse aggregation JSON")?;

            output::info(&format!("Running aggregation on namespace '{}'", namespace));
            let response = client.aggregate(namespace, request).await?;
            let json = serde_json::to_value(&response).context("Failed to serialize response")?;
            output::print_item(&json, format);
        }

        Some(("export", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let cursor = sub_matches.get_one::<String>("cursor").cloned();
            let limit = *sub_matches.get_one::<u32>("limit").unwrap();
            let include_vectors = sub_matches.get_flag("include-vectors");

            let mut request = ExportRequest::new().with_top_k(limit as usize);
            if let Some(c) = cursor {
                request = request.with_cursor(c);
            }
            if include_vectors {
                request = request.include_vectors(true);
            }

            output::info(&format!("Exporting vectors from namespace '{}'", namespace));
            let response = client.export(namespace, request).await?;
            let json = serde_json::to_value(&response).context("Failed to serialize response")?;
            output::print_item(&json, format);
        }

        Some(("explain", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let values: Vec<f32> = sub_matches.get_many::<f32>("values")
                .unwrap()
                .copied()
                .collect();
            let top_k = *sub_matches.get_one::<u32>("top-k").unwrap();
            let include_metadata = sub_matches.get_flag("include-metadata");

            let request: QueryExplainRequest = serde_json::from_str(&serde_json::json!({
                "vector": values,
                "top_k": top_k,
                "include_metadata": include_metadata,
            }).to_string()).context("Failed to build explain request")?;

            output::info(&format!("Explaining query on namespace '{}'", namespace));
            let response = client.explain_query(namespace, request).await?;
            let json = serde_json::to_value(&response).context("Failed to serialize response")?;
            output::print_item(&json, format);
        }

        Some(("upsert-columns", sub_matches)) => {
            let namespace = sub_matches.get_one::<String>("namespace").unwrap();
            let file_path = sub_matches.get_one::<String>("file").unwrap();

            let file = PathBuf::from(file_path);
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;

            let request: ColumnUpsertRequest = serde_json::from_str(&content)
                .context("Failed to parse column upsert JSON")?;

            let count = request.ids.len();
            output::info(&format!("Upserting {} vectors (column format) to namespace '{}'", count, namespace));
            client.upsert_columns(namespace, request).await?;
            output::success(&format!("Successfully upserted {} vectors (column format)", count));
        }

        _ => {
            output::error("Unknown vector subcommand. Use --help for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}
