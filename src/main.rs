//! Dakera CLI - Command-line interface for Dakera AI Agent Memory Platform

mod commands;
mod config;
mod output;

use clap::{value_parser, Arg, ArgAction, Command};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::commands::{
    admin, agent, analytics, health, index, init, keys, knowledge, memory, namespace, ops, session,
    vector,
};
use crate::config::Config;

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

fn build_cli() -> Command {
    Command::new("dk")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Dakera Team")
        .about("Dakera CLI - Manage your AI agent memory platform from the command line")
        .arg(
            Arg::new("url")
                .short('u')
                .long("url")
                .env("DAKERA_URL")
                .default_value("http://localhost:3000")
                .help("Server URL"),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .default_value("table")
                .value_parser(["table", "json", "compact"])
                .help("Output format"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("Enable verbose output"),
        )
        .subcommand(
            Command::new("init")
                .about("Interactive setup wizard — configure server URL and default namespace"),
        )
        .subcommand(
            Command::new("health")
                .about("Check server health and connectivity")
                .arg(
                    Arg::new("detailed")
                        .short('d')
                        .long("detailed")
                        .action(ArgAction::SetTrue)
                        .help("Show detailed health information"),
                ),
        )
        .subcommand(build_namespace_command())
        .subcommand(build_vector_command())
        .subcommand(build_index_command())
        .subcommand(build_ops_command())
        .subcommand(build_memory_command())
        .subcommand(build_session_command())
        .subcommand(build_agent_command())
        .subcommand(build_knowledge_command())
        .subcommand(build_analytics_command())
        .subcommand(build_admin_command())
        .subcommand(build_keys_command())
        .subcommand(
            Command::new("config")
                .about("Show or set configuration")
                .arg(
                    Arg::new("show")
                        .long("show")
                        .action(ArgAction::SetTrue)
                        .help("Show current configuration"),
                ),
        )
}

fn build_namespace_command() -> Command {
    Command::new("namespace")
        .about("Manage namespaces")
        .subcommand(Command::new("list").about("List all namespaces"))
        .subcommand(
            Command::new("get")
                .about("Get namespace information")
                .arg(Arg::new("name").required(true).help("Namespace name")),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new namespace")
                .arg(Arg::new("name").required(true).help("Namespace name"))
                .arg(
                    Arg::new("dimension")
                        .short('d')
                        .long("dimension")
                        .value_parser(value_parser!(u32))
                        .help("Vector dimension"),
                ),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete a namespace")
                .arg(Arg::new("name").required(true).help("Namespace name"))
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
}

fn build_vector_command() -> Command {
    Command::new("vector")
        .about("Manage vectors")
        .subcommand(
            Command::new("upsert")
                .about("Upsert vectors from a JSON file")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file containing vectors"),
                )
                .arg(
                    Arg::new("batch-size")
                        .short('b')
                        .long("batch-size")
                        .default_value("100")
                        .value_parser(value_parser!(usize))
                        .help("Batch size for large files"),
                ),
        )
        .subcommand(
            Command::new("upsert-one")
                .about("Upsert a single vector")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("id")
                        .short('i')
                        .long("id")
                        .required(true)
                        .help("Vector ID"),
                )
                .arg(
                    Arg::new("values")
                        .short('V')
                        .long("values")
                        .required(true)
                        .value_delimiter(',')
                        .value_parser(value_parser!(f32))
                        .help("Vector values (comma-separated floats)"),
                )
                .arg(
                    Arg::new("metadata")
                        .short('m')
                        .long("metadata")
                        .help("Optional metadata as JSON string"),
                ),
        )
        .subcommand(
            Command::new("query")
                .about("Query for similar vectors")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("values")
                        .short('V')
                        .long("values")
                        .required(true)
                        .value_delimiter(',')
                        .value_parser(value_parser!(f32))
                        .help("Query vector values (comma-separated floats)"),
                )
                .arg(
                    Arg::new("top-k")
                        .short('k')
                        .long("top-k")
                        .default_value("10")
                        .value_parser(value_parser!(u32))
                        .help("Number of results to return"),
                )
                .arg(
                    Arg::new("include-metadata")
                        .short('m')
                        .long("include-metadata")
                        .action(ArgAction::SetTrue)
                        .help("Include metadata in results"),
                )
                .arg(
                    Arg::new("filter")
                        .long("filter")
                        .help("Filter expression as JSON"),
                ),
        )
        .subcommand(
            Command::new("query-file")
                .about("Query from a file")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file containing query"),
                ),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete vectors by ID")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("ids")
                        .short('i')
                        .long("ids")
                        .value_delimiter(',')
                        .help("Vector IDs to delete"),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .action(ArgAction::SetTrue)
                        .help("Delete all vectors (dangerous!)"),
                )
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
        .subcommand(
            Command::new("multi-search")
                .about("Multi-vector search with positive/negative vectors and MMR")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file with multi-vector search request"),
                ),
        )
        .subcommand(
            Command::new("unified-query")
                .about("Unified query combining vector and text search")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file with unified query request"),
                ),
        )
        .subcommand(
            Command::new("aggregate")
                .about("Aggregate vectors with grouping")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file with aggregation request"),
                ),
        )
        .subcommand(
            Command::new("export")
                .about("Export vectors with pagination")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("cursor")
                        .short('c')
                        .long("cursor")
                        .help("Pagination cursor from previous export"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("100")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of vectors to export"),
                )
                .arg(
                    Arg::new("include-vectors")
                        .long("include-vectors")
                        .action(ArgAction::SetTrue)
                        .help("Include vector values in export"),
                ),
        )
        .subcommand(
            Command::new("explain")
                .about("Explain query execution plan")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("values")
                        .short('V')
                        .long("values")
                        .required(true)
                        .value_delimiter(',')
                        .value_parser(value_parser!(f32))
                        .help("Query vector values (comma-separated floats)"),
                )
                .arg(
                    Arg::new("top-k")
                        .short('k')
                        .long("top-k")
                        .default_value("10")
                        .value_parser(value_parser!(u32))
                        .help("Number of results to return"),
                )
                .arg(
                    Arg::new("include-metadata")
                        .short('m')
                        .long("include-metadata")
                        .action(ArgAction::SetTrue)
                        .help("Include metadata in results"),
                ),
        )
        .subcommand(
            Command::new("upsert-columns")
                .about("Column-format vector upsert from JSON file")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("JSON file with column upsert data"),
                ),
        )
}

fn build_index_command() -> Command {
    Command::new("index")
        .about("Manage indexes")
        .subcommand(
            Command::new("stats")
                .about("Get index statistics for a namespace")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                ),
        )
        .subcommand(
            Command::new("fulltext-stats")
                .about("Get full-text index statistics")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                ),
        )
        .subcommand(
            Command::new("rebuild")
                .about("Rebuild index for a namespace")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .required(true)
                        .help("Namespace name"),
                )
                .arg(
                    Arg::new("index-type")
                        .short('t')
                        .long("index-type")
                        .default_value("all")
                        .help("Index type to rebuild (vector, fulltext, all)"),
                )
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
}

fn build_ops_command() -> Command {
    Command::new("ops")
        .about("Operations and maintenance")
        .subcommand(Command::new("diagnostics").about("Get system diagnostics"))
        .subcommand(Command::new("jobs").about("List background jobs"))
        .subcommand(
            Command::new("job")
                .about("Get specific job status")
                .arg(Arg::new("id").required(true).help("Job ID")),
        )
        .subcommand(
            Command::new("compact")
                .about("Trigger index compaction")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Target namespace (optional, compacts all if not specified)"),
                )
                .arg(
                    Arg::new("force")
                        .short('f')
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Force compaction even if not needed"),
                ),
        )
        .subcommand(
            Command::new("shutdown")
                .about("Gracefully shutdown the server")
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
        .subcommand(Command::new("metrics").about("Show server metrics"))
}

fn build_memory_command() -> Command {
    Command::new("memory")
        .about("Manage agent memories")
        .subcommand(
            Command::new("store")
                .about("Store a memory for an agent")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("content")
                        .required(true)
                        .help("Memory content text"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .default_value("episodic")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Memory type"),
                )
                .arg(
                    Arg::new("importance")
                        .short('i')
                        .long("importance")
                        .default_value("0.5")
                        .value_parser(value_parser!(f32))
                        .help("Importance score (0.0 to 1.0)"),
                )
                .arg(
                    Arg::new("session-id")
                        .short('s')
                        .long("session-id")
                        .help("Session ID to associate with"),
                ),
        )
        .subcommand(
            Command::new("recall")
                .about("Recall memories by semantic query")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(Arg::new("query").required(true).help("Search query"))
                .arg(
                    Arg::new("top-k")
                        .short('k')
                        .long("top-k")
                        .default_value("5")
                        .value_parser(value_parser!(usize))
                        .help("Number of results to return"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Get a specific memory by ID")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(Arg::new("memory_id").required(true).help("Memory ID")),
        )
        .subcommand(
            Command::new("update")
                .about("Update an existing memory")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(Arg::new("memory_id").required(true).help("Memory ID"))
                .arg(
                    Arg::new("content")
                        .short('c')
                        .long("content")
                        .help("New content text"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("New memory type"),
                ),
        )
        .subcommand(
            Command::new("forget")
                .about("Delete a memory")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("memory_id")
                        .required(true)
                        .help("Memory ID to delete"),
                ),
        )
        .subcommand(
            Command::new("search")
                .about("Search memories with advanced filters")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(Arg::new("query").required(true).help("Search query"))
                .arg(
                    Arg::new("top-k")
                        .short('k')
                        .long("top-k")
                        .default_value("10")
                        .value_parser(value_parser!(usize))
                        .help("Number of results to return"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                ),
        )
        .subcommand(
            Command::new("importance")
                .about("Update importance score for memories")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("ids")
                        .long("ids")
                        .required(true)
                        .help("Comma-separated memory IDs"),
                )
                .arg(
                    Arg::new("value")
                        .long("value")
                        .required(true)
                        .value_parser(value_parser!(f32))
                        .help("New importance value (0.0 to 1.0)"),
                ),
        )
        .subcommand(
            Command::new("consolidate")
                .about("Consolidate similar memories")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                )
                .arg(
                    Arg::new("threshold")
                        .long("threshold")
                        .default_value("0.8")
                        .value_parser(value_parser!(f32))
                        .help("Similarity threshold for consolidation"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Preview consolidation without applying changes"),
                ),
        )
        .subcommand(
            Command::new("feedback")
                .about("Submit feedback on a memory recall")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(Arg::new("memory_id").required(true).help("Memory ID"))
                .arg(Arg::new("feedback").required(true).help("Feedback text"))
                .arg(
                    Arg::new("score")
                        .short('s')
                        .long("score")
                        .value_parser(value_parser!(f32))
                        .help("Relevance score (0.0 to 1.0)"),
                ),
        )
}

fn build_session_command() -> Command {
    Command::new("session")
        .about("Manage agent sessions")
        .subcommand(
            Command::new("start")
                .about("Start a new session for an agent")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("metadata")
                        .short('m')
                        .long("metadata")
                        .help("Session metadata as JSON string"),
                ),
        )
        .subcommand(
            Command::new("end")
                .about("End an active session")
                .arg(Arg::new("session_id").required(true).help("Session ID"))
                .arg(
                    Arg::new("summary")
                        .short('s')
                        .long("summary")
                        .help("Session summary text"),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Get session details")
                .arg(Arg::new("session_id").required(true).help("Session ID")),
        )
        .subcommand(
            Command::new("list")
                .about("List sessions")
                .arg(
                    Arg::new("agent-id")
                        .short('a')
                        .long("agent-id")
                        .help("Filter by agent ID"),
                )
                .arg(
                    Arg::new("active-only")
                        .long("active-only")
                        .action(ArgAction::SetTrue)
                        .help("Show only active sessions"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("50")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of sessions to return"),
                ),
        )
        .subcommand(
            Command::new("memories")
                .about("Get memories for a session")
                .arg(Arg::new("session_id").required(true).help("Session ID")),
        )
}

fn build_agent_command() -> Command {
    Command::new("agent")
        .about("Manage agents")
        .subcommand(Command::new("list").about("List all agents"))
        .subcommand(
            Command::new("memories")
                .about("Get memories for an agent")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("50")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of memories to return"),
                ),
        )
        .subcommand(
            Command::new("stats")
                .about("Get agent statistics")
                .arg(Arg::new("agent_id").required(true).help("Agent ID")),
        )
        .subcommand(
            Command::new("sessions")
                .about("Get sessions for an agent")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("active-only")
                        .long("active-only")
                        .action(ArgAction::SetTrue)
                        .help("Show only active sessions"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("50")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of sessions to return"),
                ),
        )
}

fn build_knowledge_command() -> Command {
    Command::new("knowledge")
        .about("Knowledge graph operations")
        .subcommand(
            Command::new("graph")
                .about("Build knowledge graph from a seed memory")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("memory-id")
                        .short('m')
                        .long("memory-id")
                        .help("Seed memory ID"),
                )
                .arg(
                    Arg::new("depth")
                        .short('d')
                        .long("depth")
                        .value_parser(value_parser!(u32))
                        .help("Graph traversal depth"),
                )
                .arg(
                    Arg::new("min-similarity")
                        .short('s')
                        .long("min-similarity")
                        .value_parser(value_parser!(f32))
                        .help("Minimum similarity threshold (0.0 to 1.0)"),
                ),
        )
        .subcommand(
            Command::new("full-graph")
                .about("Build full knowledge graph for an agent")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("max-nodes")
                        .long("max-nodes")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of nodes"),
                )
                .arg(
                    Arg::new("min-similarity")
                        .short('s')
                        .long("min-similarity")
                        .value_parser(value_parser!(f32))
                        .help("Minimum similarity threshold (0.0 to 1.0)"),
                )
                .arg(
                    Arg::new("cluster-threshold")
                        .long("cluster-threshold")
                        .value_parser(value_parser!(f32))
                        .help("Cluster similarity threshold (0.0 to 1.0)"),
                )
                .arg(
                    Arg::new("max-edges")
                        .long("max-edges")
                        .value_parser(value_parser!(u32))
                        .help("Maximum edges per node"),
                ),
        )
        .subcommand(
            Command::new("summarize")
                .about("Summarize agent memories")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("memory-ids")
                        .long("memory-ids")
                        .help("Comma-separated memory IDs to summarize"),
                )
                .arg(
                    Arg::new("target-type")
                        .short('t')
                        .long("target-type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Target memory type for the summary"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Preview summarization without applying changes"),
                ),
        )
        .subcommand(
            Command::new("deduplicate")
                .about("Find and remove duplicate memories")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("threshold")
                        .long("threshold")
                        .value_parser(value_parser!(f32))
                        .help("Similarity threshold for deduplication (0.0 to 1.0)"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Preview deduplication without applying changes"),
                ),
        )
}

fn build_admin_command() -> Command {
    Command::new("admin")
        .about("Cluster administration, caching, backups, and configuration")
        .subcommand(Command::new("cluster-status").about("Get cluster status overview"))
        .subcommand(Command::new("cluster-nodes").about("List cluster nodes"))
        .subcommand(
            Command::new("optimize")
                .about("Optimize a namespace (compact indexes, reclaim space)")
                .arg(
                    Arg::new("namespace")
                        .required(true)
                        .help("Namespace to optimize"),
                ),
        )
        .subcommand(
            Command::new("index-stats")
                .about("Get index statistics for a namespace")
                .arg(Arg::new("namespace").required(true).help("Namespace name")),
        )
        .subcommand(
            Command::new("rebuild-indexes")
                .about("Rebuild indexes for a namespace")
                .arg(Arg::new("namespace").required(true).help("Namespace name")),
        )
        .subcommand(Command::new("cache-stats").about("Get cache statistics"))
        .subcommand(
            Command::new("cache-clear")
                .about("Clear cache (optionally for a specific namespace)")
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Namespace to clear cache for (all if omitted)"),
                ),
        )
        .subcommand(Command::new("config-get").about("Get current server configuration"))
        .subcommand(
            Command::new("config-set")
                .about("Update a configuration value")
                .arg(
                    Arg::new("key")
                        .short('k')
                        .long("key")
                        .required(true)
                        .help("Configuration key"),
                )
                .arg(
                    Arg::new("value")
                        .short('V')
                        .long("value")
                        .required(true)
                        .help("Configuration value (string or JSON)"),
                ),
        )
        .subcommand(Command::new("quotas-get").about("List all namespace quotas"))
        .subcommand(
            Command::new("quotas-set")
                .about("Set namespace quotas")
                .arg(
                    Arg::new("data")
                        .short('d')
                        .long("data")
                        .required(true)
                        .help("Quota configuration as JSON string"),
                ),
        )
        .subcommand(
            Command::new("slow-queries")
                .about("List slow queries")
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("20")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of queries to return"),
                )
                .arg(
                    Arg::new("min-duration")
                        .long("min-duration")
                        .value_parser(value_parser!(f64))
                        .help("Minimum duration in milliseconds"),
                ),
        )
        .subcommand(
            Command::new("backup-create")
                .about("Create a new backup")
                .arg(
                    Arg::new("no-data")
                        .long("no-data")
                        .action(ArgAction::SetTrue)
                        .help("Create schema-only backup without vector data"),
                ),
        )
        .subcommand(Command::new("backup-list").about("List all backups"))
        .subcommand(
            Command::new("backup-restore")
                .about("Restore from a backup")
                .arg(
                    Arg::new("backup_id")
                        .required(true)
                        .help("Backup ID to restore"),
                ),
        )
        .subcommand(
            Command::new("backup-delete").about("Delete a backup").arg(
                Arg::new("backup_id")
                    .required(true)
                    .help("Backup ID to delete"),
            ),
        )
        .subcommand(
            Command::new("configure-ttl")
                .about("Configure TTL (time-to-live) for a namespace")
                .arg(Arg::new("namespace").required(true).help("Namespace name"))
                .arg(
                    Arg::new("ttl-seconds")
                        .long("ttl-seconds")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("TTL in seconds for vectors in this namespace"),
                )
                .arg(
                    Arg::new("strategy")
                        .long("strategy")
                        .help("TTL strategy (e.g. delete, archive)"),
                ),
        )
}

fn build_keys_command() -> Command {
    Command::new("keys")
        .about("Manage API keys")
        .subcommand(
            Command::new("create")
                .about("Create a new API key")
                .arg(
                    Arg::new("name")
                        .required(true)
                        .help("Human-readable name for the key"),
                )
                .arg(
                    Arg::new("permissions")
                        .short('p')
                        .long("permissions")
                        .help("Permission scope (e.g. read, write, admin)"),
                )
                .arg(
                    Arg::new("expires")
                        .short('e')
                        .long("expires")
                        .value_parser(value_parser!(u64))
                        .help("Expiration in days"),
                ),
        )
        .subcommand(Command::new("list").about("List all API keys"))
        .subcommand(
            Command::new("get")
                .about("Get API key details")
                .arg(Arg::new("key_id").required(true).help("API key ID")),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete (revoke) an API key")
                .arg(Arg::new("key_id").required(true).help("API key ID")),
        )
        .subcommand(
            Command::new("deactivate")
                .about("Deactivate an API key without deleting it")
                .arg(Arg::new("key_id").required(true).help("API key ID")),
        )
        .subcommand(
            Command::new("rotate")
                .about("Rotate an API key (generate new secret)")
                .arg(Arg::new("key_id").required(true).help("API key ID")),
        )
        .subcommand(
            Command::new("usage")
                .about("Get usage statistics for an API key")
                .arg(Arg::new("key_id").required(true).help("API key ID")),
        )
}

fn build_analytics_command() -> Command {
    Command::new("analytics")
        .about("View platform analytics and metrics")
        .subcommand(
            Command::new("overview")
                .about("Analytics overview")
                .arg(
                    Arg::new("period")
                        .short('p')
                        .long("period")
                        .default_value("24h")
                        .help("Time period (e.g. 1h, 24h, 7d)"),
                )
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Filter by namespace"),
                ),
        )
        .subcommand(
            Command::new("latency")
                .about("Latency statistics")
                .arg(
                    Arg::new("period")
                        .short('p')
                        .long("period")
                        .default_value("24h")
                        .help("Time period (e.g. 1h, 24h, 7d)"),
                )
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Filter by namespace"),
                ),
        )
        .subcommand(
            Command::new("throughput")
                .about("Throughput statistics")
                .arg(
                    Arg::new("period")
                        .short('p')
                        .long("period")
                        .default_value("24h")
                        .help("Time period (e.g. 1h, 24h, 7d)"),
                )
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Filter by namespace"),
                ),
        )
        .subcommand(
            Command::new("storage").about("Storage statistics").arg(
                Arg::new("namespace")
                    .short('n')
                    .long("namespace")
                    .help("Filter by namespace"),
            ),
        )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = Config::load();

    // Parse CLI arguments
    let matches = build_cli().get_matches();

    // Initialize logging if verbose
    if matches.get_flag("verbose") {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new("info"))
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    // Get URL from args or config
    let cli_url = matches.get_one::<String>("url").unwrap();
    let url = if cli_url != "http://localhost:3000" {
        cli_url.clone()
    } else {
        config.server_url.clone()
    };

    // Get output format
    let format_str = matches.get_one::<String>("format").unwrap();
    let format = OutputFormat::from(format_str.as_str());

    // Execute command
    match matches.subcommand() {
        Some(("init", _)) => {
            init::execute().await?;
        }
        Some(("health", sub_matches)) => {
            let detailed = sub_matches.get_flag("detailed");
            health::execute(&url, detailed, format).await?;
        }
        Some(("namespace", sub_matches)) => {
            namespace::execute(&url, sub_matches, format).await?;
        }
        Some(("vector", sub_matches)) => {
            vector::execute(&url, sub_matches, format).await?;
        }
        Some(("index", sub_matches)) => {
            index::execute(&url, sub_matches, format).await?;
        }
        Some(("ops", sub_matches)) => {
            ops::execute(&url, sub_matches, format).await?;
        }
        Some(("memory", sub_matches)) => {
            memory::execute(&url, sub_matches, format).await?;
        }
        Some(("session", sub_matches)) => {
            session::execute(&url, sub_matches, format).await?;
        }
        Some(("agent", sub_matches)) => {
            agent::execute(&url, sub_matches, format).await?;
        }
        Some(("knowledge", sub_matches)) => {
            knowledge::execute(&url, sub_matches, format).await?;
        }
        Some(("analytics", sub_matches)) => {
            analytics::execute(&url, sub_matches, format).await?;
        }
        Some(("admin", sub_matches)) => {
            admin::execute(&url, sub_matches, format).await?;
        }
        Some(("keys", sub_matches)) => {
            keys::execute(&url, sub_matches, format).await?;
        }
        Some(("config", _)) => {
            println!("Configuration:");
            println!("  Server URL:        {}", config.server_url);
            println!("  Default namespace: {}", config.default_namespace);
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
            println!("Run `dk init` to create or update the config file.");
        }
        _ => {
            build_cli().print_help()?;
        }
    }

    Ok(())
}
