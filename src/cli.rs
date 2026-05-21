//! Clap command tree construction.
//!
//! Every `build_*_command()` function lives here. `main.rs` imports
//! `build_cli()` and stays under 100 lines.

use clap::{value_parser, Arg, ArgAction, Command};

pub fn build_cli() -> Command {
    Command::new("dk")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Dakera Team")
        .about("Dakera CLI - Manage your AI agent memory platform from the command line")
        .after_help(
            "Examples:\n  dk health\n  dk namespace list\n  dk memory store my-agent 'Completed task X' --importance 0.8\n  dk memory recall my-agent 'recent tasks' --top-k 5\n  dk text search 'user preferences' --namespace default\n  dk completion zsh --install\n\nError exit codes:\n  0  success\n  1  general error\n  2  connection error (server unreachable)\n  3  not found\n  4  permission denied\n  5  invalid input\n  6  server error",
        )
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
                .help("Enable verbose output with HTTP request/response logging"),
        )
        .arg(
            Arg::new("profile")
                .short('p')
                .long("profile")
                .env("DAKERA_PROFILE")
                .help("Named server profile to use (overrides active_profile in config)"),
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
        .subcommand(build_index_command())
        .subcommand(build_memory_command())
        .subcommand(build_session_command())
        .subcommand(build_agent_command())
        .subcommand(build_knowledge_command())
        .subcommand(build_keys_command())
        .subcommand(build_admin_command())
        .subcommand(build_config_command())
        .subcommand(build_completion_command())
        .subcommand(build_text_command())
}

pub fn build_config_command() -> Command {
    Command::new("config")
        .about("Show configuration or manage server profiles")
        .arg(
            Arg::new("show")
                .long("show")
                .action(ArgAction::SetTrue)
                .help("Show current configuration (default action)"),
        )
        .subcommand(
            Command::new("profile")
                .about("Manage named server profiles")
                .subcommand(
                    Command::new("add")
                        .about("Add or update a named profile")
                        .arg(
                            Arg::new("name")
                                .required(true)
                                .help("Profile name (e.g. local, staging, prod)"),
                        )
                        .arg(
                            Arg::new("url")
                                .short('u')
                                .long("url")
                                .required(true)
                                .help("Server URL for this profile"),
                        )
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .help("Default namespace for this profile"),
                        ),
                )
                .subcommand(
                    Command::new("use").about("Switch the active profile").arg(
                        Arg::new("name")
                            .required(true)
                            .help("Profile name to activate"),
                    ),
                )
                .subcommand(Command::new("list").about("List all profiles")),
        )
}

pub fn build_completion_command() -> Command {
    Command::new("completion")
        .about("Generate shell completion scripts")
        .long_about(
            "Generate shell completion scripts for bash, zsh, or fish.\n\
             \n\
             Print to stdout:\n\
             \n  dk completion bash\n\
             \n  dk completion zsh\n\
             \n  dk completion fish\n\
             \nInstall automatically:\n\
             \n  dk completion bash --install\n\
             \n  dk completion zsh --install\n\
             \n  dk completion fish --install\n\
             \nDynamic completion provides namespace and agent names from the live server.",
        )
        .arg(
            Arg::new("shell")
                .required(true)
                .value_parser(["bash", "zsh", "fish"])
                .help("Shell to generate completion for"),
        )
        .arg(
            Arg::new("install")
                .long("install")
                .action(ArgAction::SetTrue)
                .help("Install the completion script to the appropriate location"),
        )
}

pub fn build_namespace_command() -> Command {
    Command::new("namespace")
        .about("Manage namespaces")
        .after_help(
            "Examples:\n  dk namespace list\n  dk namespace get my-ns\n  dk namespace create my-ns\n  dk namespace delete my-ns --dry-run\n  dk namespace delete my-ns --yes\n  dk namespace policy get my-ns\n  dk namespace policy set my-ns --consolidation-enabled true --rate-limit-enabled true --rate-limit-stores-per-minute 100",
        )
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
                .about("Delete a namespace and all its data")
                .after_help("Examples:\n  dk namespace delete my-ns --dry-run\n  dk namespace delete my-ns --yes")
                .arg(Arg::new("name").required(true).help("Namespace name"))
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Show what would be deleted without making any changes"),
                ),
        )
        .subcommand(
            Command::new("policy")
                .about("Manage namespace memory lifecycle policy (TTLs, consolidation, rate limiting)")
                .after_help("Examples:\n  dk namespace policy get my-ns\n  dk namespace policy set my-ns --consolidation-enabled true\n  dk namespace policy set my-ns --rate-limit-enabled true --rate-limit-stores-per-minute 60")
                .subcommand(
                    Command::new("get")
                        .about("Show the current memory policy for a namespace")
                        .arg(Arg::new("namespace").required(true).help("Namespace name")),
                )
                .subcommand(
                    Command::new("set")
                        .about("Update memory policy fields for a namespace (only supplied flags are changed)")
                        .arg(Arg::new("namespace").required(true).help("Namespace name"))
                        .arg(Arg::new("working-ttl").long("working-ttl").value_parser(value_parser!(u64)).help("TTL for working memories in seconds (default: 14400 = 4h)"))
                        .arg(Arg::new("episodic-ttl").long("episodic-ttl").value_parser(value_parser!(u64)).help("TTL for episodic memories in seconds (default: 2592000 = 30d)"))
                        .arg(Arg::new("semantic-ttl").long("semantic-ttl").value_parser(value_parser!(u64)).help("TTL for semantic memories in seconds (default: 31536000 = 365d)"))
                        .arg(Arg::new("procedural-ttl").long("procedural-ttl").value_parser(value_parser!(u64)).help("TTL for procedural memories in seconds (default: 63072000 = 730d)"))
                        .arg(Arg::new("working-decay").long("working-decay").value_parser(["exponential", "power_law", "logarithmic", "flat"]).help("Decay curve for working memories"))
                        .arg(Arg::new("episodic-decay").long("episodic-decay").value_parser(["exponential", "power_law", "logarithmic", "flat"]).help("Decay curve for episodic memories"))
                        .arg(Arg::new("semantic-decay").long("semantic-decay").value_parser(["exponential", "power_law", "logarithmic", "flat"]).help("Decay curve for semantic memories"))
                        .arg(Arg::new("procedural-decay").long("procedural-decay").value_parser(["exponential", "power_law", "logarithmic", "flat"]).help("Decay curve for procedural memories"))
                        .arg(Arg::new("spaced-repetition-factor").long("spaced-repetition-factor").value_parser(value_parser!(f64)).help("TTL extension multiplier per recall hit (default: 1.0; 0.0 = disabled)"))
                        .arg(Arg::new("spaced-repetition-base-interval").long("spaced-repetition-base-interval").value_parser(value_parser!(u64)).help("Base interval in seconds for spaced repetition TTL extension (default: 86400 = 1d)"))
                        .arg(Arg::new("consolidation-enabled").long("consolidation-enabled").value_parser(value_parser!(bool)).help("Enable background DBSCAN deduplication (default: false)"))
                        .arg(Arg::new("consolidation-threshold").long("consolidation-threshold").value_parser(value_parser!(f32)).help("DBSCAN cosine-similarity threshold (default: 0.92; higher = stricter)"))
                        .arg(Arg::new("consolidation-interval-hours").long("consolidation-interval-hours").value_parser(value_parser!(u32)).help("Background consolidation sweep interval in hours (default: 24)"))
                        .arg(Arg::new("rate-limit-enabled").long("rate-limit-enabled").value_parser(value_parser!(bool)).help("Enable per-namespace store/recall rate limiting (default: false)"))
                        .arg(Arg::new("rate-limit-stores-per-minute").long("rate-limit-stores-per-minute").value_parser(value_parser!(u32)).help("Max store operations per minute (omit for unlimited)"))
                        .arg(Arg::new("rate-limit-recalls-per-minute").long("rate-limit-recalls-per-minute").value_parser(value_parser!(u32)).help("Max recall operations per minute (omit for unlimited)")),
                ),
        )
}

pub fn build_index_command() -> Command {
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
                .after_help("Examples:\n  dk index rebuild -n my-ns --dry-run\n  dk index rebuild -n my-ns --index-type vector --yes")
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
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Show what would be rebuilt without making any changes"),
                ),
        )
}

pub fn build_memory_command() -> Command {
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
        .subcommand(
            Command::new("batch-forget")
                .about("Batch delete memories matching filters")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Delete memories of this type"),
                )
                .arg(
                    Arg::new("min-importance")
                        .long("min-importance")
                        .value_parser(value_parser!(f32))
                        .help("Delete memories with importance below this value"),
                )
                .arg(
                    Arg::new("max-age-days")
                        .long("max-age-days")
                        .value_parser(value_parser!(u32))
                        .help("Delete memories older than this many days"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Preview deletions without removing any memories"),
                ),
        )
}

pub fn build_session_command() -> Command {
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

pub fn build_agent_command() -> Command {
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

pub fn build_knowledge_command() -> Command {
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

pub fn build_admin_command() -> Command {
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

pub fn build_keys_command() -> Command {
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

pub fn build_text_command() -> Command {
    Command::new("text")
        .about("Full-text (BM25) search across memories")
        .subcommand(
            Command::new("search")
                .about("BM25 full-text search")
                .arg(Arg::new("query").required(true).help("Search query"))
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .help("Namespace to search in"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("10")
                        .value_parser(value_parser!(u32))
                        .help("Maximum number of results"),
                ),
        )
}
