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
        .subcommand(build_health_command())
        .subcommand(build_capabilities_command())
        .subcommand(build_attachment_command())
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

pub fn build_health_command() -> Command {
    let ready = Command::new("ready")
        .about("Readiness probe (GET /health/ready); exits 6 while the server answers 503");
    let live = Command::new("live").about("Liveness probe (GET /health/live)");
    let detailed = Arg::new("detailed")
        .short('d')
        .long("detailed")
        .action(ArgAction::SetTrue)
        .help("Show detailed health information");
    Command::new("health")
        .about("Check server health and connectivity")
        .arg(detailed)
        .subcommand(ready)
        .subcommand(live)
}

pub fn build_capabilities_command() -> Command {
    Command::new("capabilities")
        .about("Show what the server supports: models, search mode, opt-in features (v0.12+)")
}

/// Arguments shared by `attachment transcribe` and `attachment index`: the
/// memory the result becomes.
fn memory_job_args(cmd: Command) -> Command {
    let namespace = Arg::new("namespace")
        .required(true)
        .help("Namespace of the attachment");
    let reference = Arg::new("reference")
        .required(true)
        .help("Attachment reference (sha256:<hex>)");
    let agent = Arg::new("agent-id")
        .long("agent-id")
        .required(true)
        .help("Agent whose memory the result is stored as");
    let tag = Arg::new("tag")
        .long("tag")
        .action(ArgAction::Append)
        .help("Tag for the memory (repeatable)");
    let importance = Arg::new("importance")
        .long("importance")
        .value_parser(value_parser!(f64))
        .help("Importance 0.0-1.0");
    let memory_type = Arg::new("memory-type")
        .long("memory-type")
        .value_parser(["episodic", "semantic", "procedural", "working"])
        .help("Memory type");
    let session = Arg::new("session-id")
        .long("session-id")
        .help("Session to associate the memory with");
    let id = Arg::new("id").long("id").help("Custom memory id");
    let lang = Arg::new("lang")
        .long("lang")
        .help("Language of the text (ISO 639-1: en, de, fr, es, it, pt, nl)");
    let ttl = Arg::new("ttl-seconds")
        .long("ttl-seconds")
        .value_parser(value_parser!(u64))
        .help("Expire the memory after this many seconds");
    let wait = Arg::new("wait")
        .long("wait")
        .action(ArgAction::SetTrue)
        .help("Poll the job until it completes (progress goes to stderr)");
    let timeout = Arg::new("timeout")
        .long("timeout")
        .value_parser(value_parser!(u64))
        .help("With --wait: give up after this many seconds (default 600)");
    cmd.arg(namespace)
        .arg(reference)
        .arg(agent)
        .arg(tag)
        .arg(importance)
        .arg(memory_type)
        .arg(session)
        .arg(id)
        .arg(lang)
        .arg(ttl)
        .arg(wait)
        .arg(timeout)
}

fn build_attachment_transcribe_command() -> Command {
    let base = Command::new("transcribe")
        .about("Transcribe a WAV attachment into a memory (speech to text)");
    memory_job_args(base)
}

fn build_attachment_index_command() -> Command {
    let base = Command::new("index")
        .about("Index a PNG attachment as a visual memory (needs DAKERA_VISION)");
    let content = Arg::new("content")
        .long("content")
        .help("Caption stored as the memory's text");
    memory_job_args(base).arg(content)
}

fn build_attachment_job_command() -> Command {
    let namespace = Arg::new("namespace").required(true).help("Namespace");
    let reference = Arg::new("reference")
        .required(true)
        .help("Attachment reference (sha256:<hex>)");
    let job_id = Arg::new("job_id").required(true).help("Job id");
    let kind = Arg::new("kind")
        .long("kind")
        .default_value("transcribe")
        .value_parser(["transcribe", "index"])
        .help("Kind of job");
    let wait = Arg::new("wait")
        .long("wait")
        .action(ArgAction::SetTrue)
        .help("Poll until the job completes");
    let timeout = Arg::new("timeout")
        .long("timeout")
        .value_parser(value_parser!(u64))
        .help("With --wait: give up after this many seconds (default 600)");
    Command::new("job")
        .about("Show a transcription or image-index job")
        .arg(namespace)
        .arg(reference)
        .arg(job_id)
        .arg(kind)
        .arg(wait)
        .arg(timeout)
}

pub fn build_attachment_command() -> Command {
    let long_about = "Files that agent memories can point at (Dakera v0.12). Off by default on \
         the server: the routes answer 501 FEATURE_DISABLED until DAKERA_ATTACHMENTS is set \
         (and DAKERA_VISION for image indexing). `dk capabilities` shows what is on.\n\
         \n\
         A memory can only reference an attachment stored in its own namespace, \
         _dakera_agent_<agent_id>. Transcription and image jobs copy an attachment from any \
         namespace you can read into the agent's.";
    let namespace = Arg::new("namespace").required(true).help("Namespace");
    let reference = Arg::new("reference")
        .required(true)
        .help("Attachment reference (sha256:<hex>)");
    let upload = Command::new("upload")
        .about("Upload a file (the namespace is created if needed)")
        .arg(namespace.clone())
        .arg(Arg::new("file").required(true).help("File to upload"))
        .arg(
            Arg::new("content-type")
                .long("content-type")
                .help("Media type (default: guessed from the file extension)"),
        );
    let list = Command::new("list")
        .about("List the attachments of a namespace")
        .arg(namespace.clone());
    let output = Arg::new("output")
        .short('o')
        .long("output")
        .required(true)
        .help("File to write, or - for stdout");
    let download = Command::new("download")
        .about("Download an attachment's bytes")
        .arg(namespace.clone())
        .arg(reference.clone())
        .arg(output);
    let delete = Command::new("delete")
        .about("Delete an attachment (409 while a memory references it)")
        .arg(namespace)
        .arg(reference);
    Command::new("attachment")
        .visible_alias("attachments")
        .about("Attachments: upload, list, download, transcribe, index (v0.12, opt-in)")
        .long_about(long_about)
        .subcommand(upload)
        .subcommand(list)
        .subcommand(download)
        .subcommand(delete)
        .subcommand(build_attachment_transcribe_command())
        .subcommand(build_attachment_index_command())
        .subcommand(build_attachment_job_command())
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
                .about("Create a namespace (PUT /v1/namespaces/{name}); an existing one with the same dimension is left as is")
                .arg(Arg::new("name").required(true).help("Namespace name"))
                .arg(
                    Arg::new("dimension")
                        .short('d')
                        .long("dimension")
                        .required(true)
                        .value_parser(value_parser!(u32).range(1..))
                        .help("Vector dimension"),
                )
                .arg(
                    Arg::new("distance")
                        .long("distance")
                        .value_parser(["cosine", "euclidean", "dot"])
                        .help("Distance metric (server default: cosine)"),
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
                .about("Rebuild the vector index of a namespace (POST /admin/indexes/rebuild)")
                .after_help("The server picks the index per namespace: an exact flat scan at or below DAKERA_ANN_THRESHOLD, HNSW above it.\n\nExamples:\n  dk index rebuild -n my-ns --dry-run\n  dk index rebuild -n my-ns --yes\n  dk index rebuild -n my-ns --index-type hnsw --force --yes")
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
                        .help("Index kind you expect (e.g. flat, hnsw); the server rejects one that differs from its choice"),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Rebuild even when the cached index is current"),
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
                )
                .arg(memory_lang_arg("Language of the content"))
                .arg(
                    Arg::new("attachment-ref")
                        .long("attachment-ref")
                        .help("Attachment already uploaded to the agent's memory namespace (sha256:<hex>; server v0.12+, needs DAKERA_ATTACHMENTS)"),
                ),
        )
        .subcommand(build_memory_batch_store_command())
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
                )
                .arg(memory_lang_arg("Language of the query")),
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
                )
                .arg(memory_lang_arg(
                    "Language of the memory's content (a change re-derives the text-based data)",
                )),
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
                )
                .arg(memory_lang_arg("Language of the query")),
        )
        .subcommand(build_memory_extract_command())
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
                .arg(
                    Arg::new("signal")
                        .required(true)
                        .value_parser(["upvote", "downvote", "flag", "positive", "negative"])
                        .help("Feedback signal: upvote/downvote change importance, flag marks it for decay"),
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
        .subcommand(
            Command::new("batch-recall")
                .about("Filter-based memory listing by tags, importance, time range, or type (no embedding required)")
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("tags")
                        .short('T')
                        .long("tags")
                        .help("Comma-separated tags to filter by (all tags must match)"),
                )
                .arg(
                    Arg::new("min-importance")
                        .long("min-importance")
                        .value_parser(value_parser!(f32))
                        .help("Minimum importance score (0.0–1.0, inclusive)"),
                )
                .arg(
                    Arg::new("max-importance")
                        .long("max-importance")
                        .value_parser(value_parser!(f32))
                        .help("Maximum importance score (0.0–1.0, inclusive)"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Filter by memory type"),
                )
                .arg(
                    Arg::new("session-id")
                        .short('s')
                        .long("session-id")
                        .help("Filter by session ID"),
                )
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("100")
                        .value_parser(value_parser!(usize))
                        .help("Maximum number of results to return"),
                ),
        )
        .subcommand(
            Command::new("hybrid-search")
                .about("Hybrid BM25 + vector ANN search in a namespace (omit vector for BM25-only)")
                .arg(Arg::new("namespace").required(true).help("Namespace to search"))
                .arg(Arg::new("query").required(true).help("Text query"))
                .arg(
                    Arg::new("top-k")
                        .short('k')
                        .long("top-k")
                        .default_value("10")
                        .value_parser(value_parser!(u32))
                        .help("Number of results to return"),
                )
                .arg(
                    Arg::new("vector-weight")
                        .long("vector-weight")
                        .default_value("0.5")
                        .value_parser(value_parser!(f32))
                        .help("Vector weight 0.0–1.0 (0.0=BM25 only, 1.0=vector only)"),
                ),
        )
}

/// `--lang` of the `dk memory` commands (server v0.12+).
fn memory_lang_arg(help: &'static str) -> Arg {
    Arg::new("lang").long("lang").help(format!(
        "{help}: ISO 639-1 code or name, optionally with a region (pt-BR); \
         the server lists what it supports in `dk capabilities` (server v0.12+)"
    ))
}

fn build_memory_batch_store_command() -> Command {
    Command::new("batch-store")
        .about("Store many memories in one request (POST /v1/memories/store/batch, up to 1000)")
        .after_help(
            "The --file input is a JSON array. Each item is either a string (the content) or an \
             object with `content` and optional `memory_type`, `importance`, `tags`, \
             `session_id`, `metadata`, `ttl_seconds`, `expires_at`, `valid_from`, `id` and \
             `attachment_ref`. --type, --importance, --tag and --session-id fill in what an \
             item does not set.\n\nExamples:\n  dk memory batch-store my-agent -c 'Prefers dark mode' -c 'Lives in Berlin'\n  dk memory batch-store my-agent --file memories.json --lang de\n  cat memories.json | dk memory batch-store my-agent --file -",
        )
        .arg(Arg::new("agent_id").required(true).help("Agent ID"))
        .arg(
            Arg::new("content")
                .short('c')
                .long("content")
                .action(ArgAction::Append)
                .help("Memory content (repeatable)"),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .help("JSON array of memories ('-' reads stdin)"),
        )
        .arg(
            Arg::new("type")
                .short('t')
                .long("type")
                .value_parser(["episodic", "semantic", "procedural", "working"])
                .help("Memory type for items that do not set one (default episodic)"),
        )
        .arg(
            Arg::new("importance")
                .short('i')
                .long("importance")
                .value_parser(value_parser!(f32))
                .help("Importance for items that do not set one (default 0.5)"),
        )
        .arg(
            Arg::new("tag")
                .long("tag")
                .action(ArgAction::Append)
                .help("Tag for items that set no tags (repeatable)"),
        )
        .arg(
            Arg::new("session-id")
                .short('s')
                .long("session-id")
                .help("Session for items that do not set one"),
        )
        .arg(memory_lang_arg("Language of every item's content"))
}

fn build_memory_extract_command() -> Command {
    Command::new("extract")
        .about("Extract entities from text without storing it (POST /v1/memories/extract)")
        .arg(Arg::new("text").required(true).help("Text to extract entities from"))
        .arg(
            Arg::new("entity-types")
                .short('e')
                .long("entity-types")
                .value_delimiter(',')
                .help("Entity types for the neural extractor, comma-separated (default: the server's person, organization, location)"),
        )
        .arg(memory_lang_arg("Language of the text"))
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
                        .required(true)
                        .help("Seed memory ID (the server builds the graph around it)"),
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
                .about(
                    "Summarize a group of memories into a new memory (the server always stores it)",
                )
                .arg(Arg::new("agent_id").required(true).help("Agent ID"))
                .arg(
                    Arg::new("memory-ids")
                        .long("memory-ids")
                        .required(true)
                        .help("Comma-separated memory IDs to summarize (at least two)"),
                )
                .arg(
                    Arg::new("target-type")
                        .short('t')
                        .long("target-type")
                        .value_parser(["episodic", "semantic", "procedural", "working"])
                        .help("Target memory type for the summary"),
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
        .subcommand(build_quotas_set_command())
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
        .subcommand(build_backup_create_command())
        .subcommand(Command::new("backup-list").about("List all backups"))
        .subcommand(build_backup_get_command())
        .subcommand(build_backup_download_command())
        .subcommand(build_backup_upload_command())
        .subcommand(build_backup_restore_command())
        .subcommand(build_backup_restore_status_command())
        .subcommand(build_backup_schedule_command())
        .subcommand(build_encryption_status_command())
        .subcommand(build_encryption_rotate_command())
        .subcommand(build_encryption_reseal_command())
        .subcommand(build_embed_migration_command())
        .subcommand(
            Command::new("backup-delete").about("Delete a backup").arg(
                Arg::new("backup_id")
                    .required(true)
                    .help("Backup ID to delete"),
            ),
        )
}

fn build_quotas_set_command() -> Command {
    let namespace = Arg::new("namespace")
        .short('n')
        .long("namespace")
        .help("Namespace to set the quota of (default: the default quota)");
    let data = Arg::new("data")
        .short('d')
        .long("data")
        .required(true)
        .help("Quota config as JSON, e.g. {\"max_vectors\": 100000}");
    Command::new("quotas-set")
        .about("Set a namespace quota (a hard quota is enforced since v0.12: 413)")
        .arg(namespace)
        .arg(data)
}

fn wait_arg() -> Arg {
    Arg::new("wait")
        .long("wait")
        .action(ArgAction::SetTrue)
        .help("Poll until the background job ends")
}

fn timeout_arg() -> Arg {
    Arg::new("timeout")
        .long("timeout")
        .value_parser(value_parser!(u64))
        .help("With --wait: give up after this many seconds (default 3600)")
}

fn backup_id_arg() -> Arg {
    Arg::new("backup_id").required(true).help("Backup id")
}

fn namespace_flag(help: &'static str) -> Arg {
    Arg::new("namespace")
        .short('n')
        .long("namespace")
        .action(ArgAction::Append)
        .help(help)
}

fn build_backup_create_command() -> Command {
    let name = Arg::new("name")
        .long("name")
        .help("Backup name (default: dk-backup-<unix time>)");
    let kind = Arg::new("type")
        .long("type")
        .value_parser(["full", "incremental", "snapshot"])
        .help("Backup type (server default: full)");
    let encrypt = Arg::new("encrypt")
        .long("encrypt")
        .action(ArgAction::SetTrue)
        .help("Encrypt the backup (needs DAKERA_ENCRYPTION_KEY on the server)");
    let compression = Arg::new("compression")
        .long("compression")
        .value_parser(["none", "zstd", "lz4"])
        .help("Compression of the backup files");
    Command::new("backup-create")
        .about("Create a backup (runs in the background; needs a global admin key)")
        .arg(name)
        .arg(kind)
        .arg(namespace_flag(
            "Back up only this namespace (repeatable; default: all)",
        ))
        .arg(encrypt)
        .arg(compression)
        .arg(wait_arg())
        .arg(timeout_arg())
}

fn build_backup_get_command() -> Command {
    Command::new("backup-get")
        .about("Show one backup (status, size, namespaces)")
        .arg(backup_id_arg())
}

fn build_backup_download_command() -> Command {
    let output = Arg::new("output")
        .short('o')
        .long("output")
        .required(true)
        .help("File to write (a .json.gz bundle), or - for stdout");
    Command::new("backup-download")
        .about("Download a backup bundle (needs a global super_admin key)")
        .arg(backup_id_arg())
        .arg(output)
}

fn build_backup_upload_command() -> Command {
    let file = Arg::new("file")
        .required(true)
        .help("Bundle from backup-download (.json.gz or .json)");
    Command::new("backup-upload")
        .about("Upload a backup bundle and restore it (needs a global super_admin key)")
        .arg(file)
}

fn build_backup_restore_command() -> Command {
    let overwrite = Arg::new("overwrite")
        .long("overwrite")
        .action(ArgAction::SetTrue)
        .help("Restore the point in time: remove what was written since the backup");
    let yes = Arg::new("yes")
        .long("yes")
        .action(ArgAction::SetTrue)
        .help("Confirm --overwrite");
    Command::new("backup-restore")
        .about("Restore from a backup (needs a global super_admin key)")
        .arg(backup_id_arg())
        .arg(namespace_flag(
            "Restore only this namespace (repeatable; default: all)",
        ))
        .arg(overwrite)
        .arg(yes)
        .arg(wait_arg())
        .arg(timeout_arg())
}

fn build_backup_restore_status_command() -> Command {
    let restore = Arg::new("restore_id").required(true).help("Restore id");
    Command::new("backup-restore-status")
        .about("Show the progress of a restore")
        .arg(restore)
}

fn build_backup_schedule_command() -> Command {
    let set = Arg::new("set")
        .long("set")
        .help("Update the schedule from this JSON (enabled, cron, backup_type, ...)");
    Command::new("backup-schedule")
        .about("Show the backup schedule (an enabled schedule runs since v0.12)")
        .arg(set)
}

fn build_encryption_status_command() -> Command {
    Command::new("encryption-status")
        .about("Show the encryption keyring and the background re-seal (global admin key)")
}

fn wait_secs_arg() -> Arg {
    Arg::new("wait-secs")
        .long("wait-secs")
        .value_parser(value_parser!(u64))
        .help("Wait this long (at most 300) for the re-seal before answering")
}

fn build_encryption_rotate_command() -> Command {
    let key_env = Arg::new("new-key-env")
        .long("new-key-env")
        .help("Environment variable holding the new passphrase or 64-char hex key");
    Command::new("encryption-rotate")
        .about("Rotate the encryption key, of one namespace or of everything")
        .arg(namespace_flag_single(
            "Rotate only this namespace (default: all)",
        ))
        .arg(key_env)
        .arg(wait_secs_arg())
}

fn namespace_flag_single(help: &'static str) -> Arg {
    Arg::new("namespace")
        .short('n')
        .long("namespace")
        .help(help)
}

fn build_encryption_reseal_command() -> Command {
    Command::new("encryption-reseal")
        .about("Run a re-seal pass now (legacy values and values under retired keys)")
        .arg(namespace_flag_single("Re-seal only this namespace"))
        .arg(wait_secs_arg())
}

fn build_embed_migration_command() -> Command {
    Command::new("embed-migration")
        .about("Show the background re-embed after an upgrade to v0.12 (global admin key)")
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
                        .help("Namespace to search (required)"),
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
