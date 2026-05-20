//! `dk completion` — shell completion script generator

use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::output;

/// All top-level `dk` subcommands (used in every shell script).
const TOP_LEVEL_CMDS: &str =
    "init health namespace vector index ops memory session agent knowledge analytics admin keys config completion";

// ─── Bash ────────────────────────────────────────────────────────────────────

fn bash_script() -> String {
    format!(
        r#"# bash completion for dk                              -*- shell-script -*-
# Source this file or place it in /etc/bash_completion.d/ or
# ~/.local/share/bash-completion/completions/dk

_dk_complete_namespaces() {{
    if command -v jq >/dev/null 2>&1; then
        dk namespace list --format json 2>/dev/null | jq -r '.[].name' 2>/dev/null
    elif command -v python3 >/dev/null 2>&1; then
        dk namespace list --format json 2>/dev/null \
            | python3 -c "import json,sys; [print(n['name']) for n in json.load(sys.stdin)]" 2>/dev/null
    fi
}}

_dk_complete_agents() {{
    if command -v jq >/dev/null 2>&1; then
        dk agent list --format json 2>/dev/null | jq -r '.[].agent_id' 2>/dev/null
    elif command -v python3 >/dev/null 2>&1; then
        dk agent list --format json 2>/dev/null \
            | python3 -c "import json,sys; [print(n['agent_id']) for n in json.load(sys.stdin)]" 2>/dev/null
    fi
}}

_dk() {{
    local cur prev words cword
    _init_completion 2>/dev/null || {{
        COMPREPLY=()
        cur="${{COMP_WORDS[COMP_CWORD]}}"
        prev="${{COMP_WORDS[COMP_CWORD-1]}}"
        words=("${{COMP_WORDS[@]}}")
        cword=$COMP_CWORD
    }}

    # Handle option arguments
    case "$prev" in
        --namespace|-n)
            COMPREPLY=($(compgen -W "$(_dk_complete_namespaces)" -- "$cur"))
            return 0
            ;;
        --agent-id)
            COMPREPLY=($(compgen -W "$(_dk_complete_agents)" -- "$cur"))
            return 0
            ;;
        --format|-f)
            COMPREPLY=($(compgen -W "table json compact" -- "$cur"))
            return 0
            ;;
        --url|-u|--output|-o|--file|--query|--seed|--type|--index-type \
        |--limit|--top-k|--dimension|--importance|--period|--threshold \
        |--agent-a|--agent-b|--key|--value|--name|--description)
            # these take a free-form argument — no completion
            return 0
            ;;
    esac

    # Determine the active subcommand (first non-flag word after "dk")
    local cmd=""
    local sub=""
    local i=1
    while [[ $i -lt $cword ]]; do
        local w="${{words[$i]}}"
        if [[ "$w" != -* ]]; then
            if [[ -z "$cmd" ]]; then
                cmd="$w"
            elif [[ -z "$sub" ]]; then
                sub="$w"
            fi
        fi
        ((i++))
    done

    if [[ -z "$cmd" ]]; then
        COMPREPLY=($(compgen -W "{TOP_LEVEL_CMDS}" -- "$cur"))
        return 0
    fi

    case "$cmd" in
        namespace)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W "list get create delete" -- "$cur"))
            ;;
        vector)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "upsert upsert-one query query-file delete multi-search unified-query aggregate export explain upsert-columns" \
                -- "$cur"))
            ;;
        index)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W "stats fulltext-stats rebuild" -- "$cur"))
            ;;
        ops)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "diagnostics jobs job compact shutdown metrics" -- "$cur"))
            ;;
        memory)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "store recall get update forget search importance consolidate feedback" -- "$cur"))
            ;;
        session)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W "start end get list memories" -- "$cur"))
            ;;
        agent)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W "list memories stats sessions" -- "$cur"))
            ;;
        knowledge)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "graph full-graph summarize deduplicate" -- "$cur"))
            ;;
        analytics)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "overview latency throughput storage" -- "$cur"))
            ;;
        admin)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "cluster-status cluster-nodes optimize index-stats rebuild-indexes \
                 cache-stats cache-clear config-get config-set quotas-get quotas-set \
                 slow-queries backup-create backup-list backup-restore backup-delete configure-ttl" \
                -- "$cur"))
            ;;
        keys)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W \
                "create list get delete deactivate rotate usage" -- "$cur"))
            ;;
        completion)
            [[ -z "$sub" ]] && COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            ;;
        health|init|config)
            # no subcommands
            ;;
    esac

    return 0
}}

complete -F _dk dk
"#,
        TOP_LEVEL_CMDS = TOP_LEVEL_CMDS
    )
}

// ─── Zsh ─────────────────────────────────────────────────────────────────────

fn zsh_script() -> &'static str {
    r#"#compdef dk
# zsh completion for dk
# Place this file as _dk in a directory on your $fpath, e.g. ~/.zfunc/_dk
# Then add: fpath=(~/.zfunc $fpath) and autoload -Uz compinit && compinit

_dk_namespaces() {
    local -a ns
    if command -v jq >/dev/null 2>&1; then
        ns=(${(f)"$(dk namespace list --format json 2>/dev/null | jq -r '.[].name' 2>/dev/null)"})
    elif command -v python3 >/dev/null 2>&1; then
        ns=(${(f)"$(dk namespace list --format json 2>/dev/null \
            | python3 -c "import json,sys; [print(n['name']) for n in json.load(sys.stdin)]" 2>/dev/null)"})
    fi
    _describe 'namespace' ns
}

_dk_agents() {
    local -a agents
    if command -v jq >/dev/null 2>&1; then
        agents=(${(f)"$(dk agent list --format json 2>/dev/null | jq -r '.[].agent_id' 2>/dev/null)"})
    elif command -v python3 >/dev/null 2>&1; then
        agents=(${(f)"$(dk agent list --format json 2>/dev/null \
            | python3 -c "import json,sys; [print(n['agent_id']) for n in json.load(sys.stdin)]" 2>/dev/null)"})
    fi
    _describe 'agent' agents
}

_dk() {
    local context state line
    typeset -A opt_args

    _arguments -C \
        '(-u --url)'{-u,--url}'[Server URL]:url:' \
        '(-f --format)'{-f,--format}'[Output format]:format:(table json compact)' \
        '(-v --verbose)'{-v,--verbose}'[Enable verbose output]' \
        '1: :->cmd' \
        '*: :->args'

    case $state in
        cmd)
            local commands=(
                'init:Interactive setup wizard'
                'health:Check server health'
                'namespace:Manage namespaces'
                'vector:Vector operations'
                'index:Index management'
                'ops:Operations and diagnostics'
                'memory:Memory operations'
                'session:Session management'
                'agent:Agent management'
                'knowledge:Knowledge graph operations'
                'analytics:Analytics and statistics'
                'admin:Administrative operations'
                'keys:API key management'
                'config:Show or set configuration'
                'completion:Generate shell completion scripts'
            )
            _describe 'command' commands
            ;;
        args)
            case ${line[1]} in
                namespace)
                    local ns_cmds=(
                        'list:List all namespaces'
                        'get:Get namespace information'
                        'create:Create a new namespace'
                        'delete:Delete a namespace'
                    )
                    _arguments '1: :->subcmd' '*: :->ns_args'
                    case $state in
                        subcmd) _describe 'namespace subcommand' ns_cmds ;;
                        ns_args)
                            case ${line[1]} in
                                get|create|delete) _message 'namespace name' ;;
                            esac
                            ;;
                    esac
                    ;;
                vector)
                    local v_cmds=(
                        'upsert:Upsert vectors from JSON file'
                        'upsert-one:Upsert a single vector'
                        'query:Query for similar vectors'
                        'query-file:Query from file'
                        'delete:Delete vectors by ID'
                        'multi-search:Multi-vector search with MMR'
                        'unified-query:Combined vector and text search'
                        'aggregate:Aggregate vectors with grouping'
                        'export:Export vectors with pagination'
                        'explain:Explain query execution plan'
                        'upsert-columns:Column-format vector upsert'
                    )
                    _arguments '1: :->subcmd' '*:namespace:_dk_namespaces'
                    [[ $state == subcmd ]] && _describe 'vector subcommand' v_cmds
                    ;;
                index)
                    local i_cmds=(
                        'stats:Get index statistics'
                        'fulltext-stats:Get full-text index statistics'
                        'rebuild:Rebuild index'
                    )
                    _arguments '1: :->subcmd'
                    [[ $state == subcmd ]] && _describe 'index subcommand' i_cmds
                    ;;
                ops)
                    local ops_cmds=(
                        'diagnostics:Get system diagnostics'
                        'jobs:List background jobs'
                        'job:Get specific job status'
                        'compact:Trigger index compaction'
                        'shutdown:Gracefully shutdown server'
                        'metrics:Show server metrics'
                    )
                    _arguments '1: :->subcmd'
                    [[ $state == subcmd ]] && _describe 'ops subcommand' ops_cmds
                    ;;
                memory)
                    local m_cmds=(
                        'store:Store a memory'
                        'recall:Recall memories by semantic query'
                        'get:Get a memory by ID'
                        'update:Update an existing memory'
                        'forget:Delete a memory'
                        'search:Search memories with filters'
                        'importance:Update importance score'
                        'consolidate:Consolidate similar memories'
                        'feedback:Submit recall feedback'
                    )
                    _arguments '1: :->subcmd' \
                        '(--namespace -n)'{--namespace,-n}'[Namespace]:namespace:_dk_namespaces' \
                        '--agent-id[Agent ID]:agent:_dk_agents'
                    [[ $state == subcmd ]] && _describe 'memory subcommand' m_cmds
                    ;;
                session)
                    local s_cmds=(
                        'start:Start a new session'
                        'end:End active session'
                        'get:Get session details'
                        'list:List sessions'
                        'memories:Get memories for session'
                    )
                    _arguments '1: :->subcmd' \
                        '--agent-id[Agent ID]:agent:_dk_agents'
                    [[ $state == subcmd ]] && _describe 'session subcommand' s_cmds
                    ;;
                agent)
                    local a_cmds=(
                        'list:List all agents'
                        'memories:Get memories for agent'
                        'stats:Get agent statistics'
                        'sessions:Get sessions for agent'
                    )
                    _arguments '1: :->subcmd'
                    [[ $state == subcmd ]] && _describe 'agent subcommand' a_cmds
                    ;;
                knowledge)
                    local k_cmds=(
                        'graph:Build knowledge graph from seed memory'
                        'full-graph:Build full knowledge graph'
                        'summarize:Summarize agent memories'
                        'deduplicate:Find and remove duplicate memories'
                    )
                    _arguments '1: :->subcmd' \
                        '--agent-id[Agent ID]:agent:_dk_agents'
                    [[ $state == subcmd ]] && _describe 'knowledge subcommand' k_cmds
                    ;;
                analytics)
                    local an_cmds=(
                        'overview:Analytics overview'
                        'latency:Latency statistics'
                        'throughput:Throughput statistics'
                        'storage:Storage statistics'
                    )
                    _arguments '1: :->subcmd' \
                        '(--namespace -n)'{--namespace,-n}'[Namespace]:namespace:_dk_namespaces'
                    [[ $state == subcmd ]] && _describe 'analytics subcommand' an_cmds
                    ;;
                admin)
                    local ad_cmds=(
                        'cluster-status:Get cluster status'
                        'cluster-nodes:List cluster nodes'
                        'optimize:Optimize namespace'
                        'index-stats:Get index statistics'
                        'rebuild-indexes:Rebuild namespace indexes'
                        'cache-stats:Get cache statistics'
                        'cache-clear:Clear cache'
                        'config-get:Get server configuration'
                        'config-set:Update configuration'
                        'quotas-get:List namespace quotas'
                        'quotas-set:Set namespace quotas'
                        'slow-queries:List slow queries'
                        'backup-create:Create backup'
                        'backup-list:List backups'
                        'backup-restore:Restore from backup'
                        'backup-delete:Delete backup'
                        'configure-ttl:Configure TTL for namespace'
                    )
                    _arguments '1: :->subcmd' \
                        '(--namespace -n)'{--namespace,-n}'[Namespace]:namespace:_dk_namespaces'
                    [[ $state == subcmd ]] && _describe 'admin subcommand' ad_cmds
                    ;;
                keys)
                    local ky_cmds=(
                        'create:Create new API key'
                        'list:List all keys'
                        'get:Get key details'
                        'delete:Delete/revoke key'
                        'deactivate:Deactivate key'
                        'rotate:Rotate key'
                        'usage:Get usage statistics'
                    )
                    _arguments '1: :->subcmd'
                    [[ $state == subcmd ]] && _describe 'keys subcommand' ky_cmds
                    ;;
                completion)
                    _arguments '1:shell:(bash zsh fish)' \
                        '--install[Install completion script]'
                    ;;
                health)
                    _arguments '--detailed[Show detailed health info]'
                    ;;
            esac
            ;;
    esac
}

_dk "$@"
"#
}

// ─── Fish ────────────────────────────────────────────────────────────────────

fn fish_script() -> &'static str {
    r#"# fish completion for dk
# Place this file at ~/.config/fish/completions/dk.fish

function __dk_no_subcommand
    for i in (commandline -opc)
        if contains -- $i init health namespace vector index ops memory session agent \
                       knowledge analytics admin keys config completion
            return 1
        end
    end
    return 0
end

function __dk_using_subcommand
    set -l cmd (commandline -opc)
    for i in $cmd
        if contains -- $i $argv
            return 0
        end
    end
    return 1
end

function __dk_namespaces
    dk namespace list --format json 2>/dev/null \
        | string match -r '"name"\s*:\s*"([^"]+)"' --groups-only 2>/dev/null
end

function __dk_agents
    dk agent list --format json 2>/dev/null \
        | string match -r '"agent_id"\s*:\s*"([^"]+)"' --groups-only 2>/dev/null
end

# Global flags
complete -c dk -s u -l url       -d 'Server URL' -r
complete -c dk -s f -l format    -d 'Output format' -r -a 'table json compact'
complete -c dk -s v -l verbose   -d 'Enable verbose output'

# Top-level subcommands
complete -c dk -f -n '__dk_no_subcommand' -a 'init'       -d 'Interactive setup wizard'
complete -c dk -f -n '__dk_no_subcommand' -a 'health'     -d 'Check server health'
complete -c dk -f -n '__dk_no_subcommand' -a 'namespace'  -d 'Manage namespaces'
complete -c dk -f -n '__dk_no_subcommand' -a 'vector'     -d 'Vector operations'
complete -c dk -f -n '__dk_no_subcommand' -a 'index'      -d 'Index management'
complete -c dk -f -n '__dk_no_subcommand' -a 'ops'        -d 'Operations and diagnostics'
complete -c dk -f -n '__dk_no_subcommand' -a 'memory'     -d 'Memory operations'
complete -c dk -f -n '__dk_no_subcommand' -a 'session'    -d 'Session management'
complete -c dk -f -n '__dk_no_subcommand' -a 'agent'      -d 'Agent management'
complete -c dk -f -n '__dk_no_subcommand' -a 'knowledge'  -d 'Knowledge graph operations'
complete -c dk -f -n '__dk_no_subcommand' -a 'analytics'  -d 'Analytics and statistics'
complete -c dk -f -n '__dk_no_subcommand' -a 'admin'      -d 'Administrative operations'
complete -c dk -f -n '__dk_no_subcommand' -a 'keys'       -d 'API key management'
complete -c dk -f -n '__dk_no_subcommand' -a 'config'     -d 'Show or set configuration'
complete -c dk -f -n '__dk_no_subcommand' -a 'completion' -d 'Generate shell completion scripts'

# namespace subcommands
complete -c dk -f -n '__dk_using_subcommand namespace' -a 'list'   -d 'List all namespaces'
complete -c dk -f -n '__dk_using_subcommand namespace' -a 'get'    -d 'Get namespace information'
complete -c dk -f -n '__dk_using_subcommand namespace' -a 'create' -d 'Create a new namespace'
complete -c dk -f -n '__dk_using_subcommand namespace' -a 'delete' -d 'Delete a namespace'

# vector subcommands
complete -c dk -f -n '__dk_using_subcommand vector' -a 'upsert'        -d 'Upsert vectors from JSON file'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'upsert-one'    -d 'Upsert a single vector'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'query'         -d 'Query for similar vectors'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'query-file'    -d 'Query from file'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'delete'        -d 'Delete vectors by ID'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'multi-search'  -d 'Multi-vector search with MMR'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'unified-query' -d 'Combined vector and text search'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'aggregate'     -d 'Aggregate vectors with grouping'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'export'        -d 'Export vectors with pagination'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'explain'       -d 'Explain query execution plan'
complete -c dk -f -n '__dk_using_subcommand vector' -a 'upsert-columns' -d 'Column-format vector upsert'

# index subcommands
complete -c dk -f -n '__dk_using_subcommand index' -a 'stats'         -d 'Get index statistics'
complete -c dk -f -n '__dk_using_subcommand index' -a 'fulltext-stats' -d 'Get full-text index statistics'
complete -c dk -f -n '__dk_using_subcommand index' -a 'rebuild'       -d 'Rebuild index'

# ops subcommands
complete -c dk -f -n '__dk_using_subcommand ops' -a 'diagnostics' -d 'Get system diagnostics'
complete -c dk -f -n '__dk_using_subcommand ops' -a 'jobs'        -d 'List background jobs'
complete -c dk -f -n '__dk_using_subcommand ops' -a 'job'         -d 'Get specific job status'
complete -c dk -f -n '__dk_using_subcommand ops' -a 'compact'     -d 'Trigger index compaction'
complete -c dk -f -n '__dk_using_subcommand ops' -a 'shutdown'    -d 'Gracefully shutdown server'
complete -c dk -f -n '__dk_using_subcommand ops' -a 'metrics'     -d 'Show server metrics'

# memory subcommands
complete -c dk -f -n '__dk_using_subcommand memory' -a 'store'       -d 'Store a memory'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'recall'      -d 'Recall memories by semantic query'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'get'         -d 'Get a memory by ID'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'update'      -d 'Update an existing memory'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'forget'      -d 'Delete a memory'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'search'      -d 'Search memories with filters'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'importance'  -d 'Update importance score'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'consolidate' -d 'Consolidate similar memories'
complete -c dk -f -n '__dk_using_subcommand memory' -a 'feedback'    -d 'Submit recall feedback'
complete -c dk -n '__dk_using_subcommand memory' -l namespace -s n -d 'Namespace' -r -a '(__dk_namespaces)'
complete -c dk -n '__dk_using_subcommand memory' -l agent-id  -d 'Agent ID' -r -a '(__dk_agents)'

# session subcommands
complete -c dk -f -n '__dk_using_subcommand session' -a 'start'    -d 'Start a new session'
complete -c dk -f -n '__dk_using_subcommand session' -a 'end'      -d 'End active session'
complete -c dk -f -n '__dk_using_subcommand session' -a 'get'      -d 'Get session details'
complete -c dk -f -n '__dk_using_subcommand session' -a 'list'     -d 'List sessions'
complete -c dk -f -n '__dk_using_subcommand session' -a 'memories' -d 'Get memories for session'
complete -c dk -n '__dk_using_subcommand session' -l agent-id -d 'Agent ID' -r -a '(__dk_agents)'

# agent subcommands
complete -c dk -f -n '__dk_using_subcommand agent' -a 'list'     -d 'List all agents'
complete -c dk -f -n '__dk_using_subcommand agent' -a 'memories' -d 'Get memories for agent'
complete -c dk -f -n '__dk_using_subcommand agent' -a 'stats'    -d 'Get agent statistics'
complete -c dk -f -n '__dk_using_subcommand agent' -a 'sessions' -d 'Get sessions for agent'

# knowledge subcommands
complete -c dk -f -n '__dk_using_subcommand knowledge' -a 'graph'        -d 'Build knowledge graph from seed'
complete -c dk -f -n '__dk_using_subcommand knowledge' -a 'full-graph'   -d 'Build full knowledge graph'
complete -c dk -f -n '__dk_using_subcommand knowledge' -a 'summarize'    -d 'Summarize agent memories'
complete -c dk -f -n '__dk_using_subcommand knowledge' -a 'deduplicate'  -d 'Find and remove duplicate memories'
complete -c dk -n '__dk_using_subcommand knowledge' -l agent-id -d 'Agent ID' -r -a '(__dk_agents)'

# analytics subcommands
complete -c dk -f -n '__dk_using_subcommand analytics' -a 'overview'    -d 'Analytics overview'
complete -c dk -f -n '__dk_using_subcommand analytics' -a 'latency'     -d 'Latency statistics'
complete -c dk -f -n '__dk_using_subcommand analytics' -a 'throughput'  -d 'Throughput statistics'
complete -c dk -f -n '__dk_using_subcommand analytics' -a 'storage'     -d 'Storage statistics'
complete -c dk -n '__dk_using_subcommand analytics' -l namespace -s n -d 'Namespace' -r -a '(__dk_namespaces)'

# admin subcommands
complete -c dk -f -n '__dk_using_subcommand admin' -a 'cluster-status'  -d 'Get cluster status'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'cluster-nodes'   -d 'List cluster nodes'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'optimize'        -d 'Optimize namespace'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'index-stats'     -d 'Get index statistics'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'rebuild-indexes' -d 'Rebuild namespace indexes'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'cache-stats'     -d 'Get cache statistics'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'cache-clear'     -d 'Clear cache'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'config-get'      -d 'Get server configuration'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'config-set'      -d 'Update configuration'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'quotas-get'      -d 'List namespace quotas'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'quotas-set'      -d 'Set namespace quotas'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'slow-queries'    -d 'List slow queries'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'backup-create'   -d 'Create backup'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'backup-list'     -d 'List backups'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'backup-restore'  -d 'Restore from backup'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'backup-delete'   -d 'Delete backup'
complete -c dk -f -n '__dk_using_subcommand admin' -a 'configure-ttl'   -d 'Configure TTL for namespace'
complete -c dk -n '__dk_using_subcommand admin' -l namespace -s n -d 'Namespace' -r -a '(__dk_namespaces)'

# keys subcommands
complete -c dk -f -n '__dk_using_subcommand keys' -a 'create'     -d 'Create new API key'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'list'       -d 'List all keys'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'get'        -d 'Get key details'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'delete'     -d 'Delete/revoke key'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'deactivate' -d 'Deactivate key'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'rotate'     -d 'Rotate key'
complete -c dk -f -n '__dk_using_subcommand keys' -a 'usage'      -d 'Get usage statistics'

# completion subcommands
complete -c dk -f -n '__dk_using_subcommand completion' -a 'bash' -d 'Generate bash completion'
complete -c dk -f -n '__dk_using_subcommand completion' -a 'zsh'  -d 'Generate zsh completion'
complete -c dk -f -n '__dk_using_subcommand completion' -a 'fish' -d 'Generate fish completion'
complete -c dk -n '__dk_using_subcommand completion' -l install -d 'Install completion script'

# health flags
complete -c dk -n '__dk_using_subcommand health' -l detailed -d 'Show detailed health info'
"#
}

// ─── Install paths ────────────────────────────────────────────────────────────

fn bash_install_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    // Prefer XDG bash-completion user directory
    let xdg = home.join(".local/share/bash-completion/completions/dk");
    Ok(xdg)
}

fn zsh_install_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".zfunc/_dk"))
}

fn fish_install_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config/fish/completions/dk.fish"))
}

fn write_completion(path: &PathBuf, script: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    f.write_all(script.as_bytes())?;
    Ok(())
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn execute(shell: &str, install: bool) -> Result<()> {
    match shell {
        "bash" => {
            let script = bash_script();
            if install {
                let path = bash_install_path()?;
                write_completion(&path, &script)?;
                output::success(&format!("Bash completion installed to {}", path.display()));
                println!();
                println!("To activate in the current shell:");
                println!("  source {}", path.display());
                println!();
                println!("It will load automatically in new shells that use bash-completion.");
            } else {
                print!("{}", script);
            }
        }
        "zsh" => {
            let script = zsh_script();
            if install {
                let path = zsh_install_path()?;
                write_completion(&path, script)?;
                output::success(&format!("Zsh completion installed to {}", path.display()));
                println!();
                println!("To activate, ensure these lines are in your ~/.zshrc:");
                println!("  fpath=(~/.zfunc $fpath)");
                println!("  autoload -Uz compinit && compinit");
                println!();
                println!("Then reload: exec zsh");
            } else {
                print!("{}", script);
            }
        }
        "fish" => {
            let script = fish_script();
            if install {
                let path = fish_install_path()?;
                write_completion(&path, script)?;
                output::success(&format!("Fish completion installed to {}", path.display()));
                println!();
                println!("Completion is active immediately in new fish sessions.");
                println!("To reload in the current session:");
                println!("  source {}", path.display());
            } else {
                print!("{}", script);
            }
        }
        other => {
            bail!(
                "unknown shell '{}'. Supported shells: bash, zsh, fish",
                other
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::build_completion_command;

    #[test]
    fn completion_requires_shell_argument() {
        assert!(
            build_completion_command()
                .try_get_matches_from(["completion"])
                .is_err(),
            "completion without shell argument should fail"
        );
    }

    #[test]
    fn completion_bash_is_valid() {
        build_completion_command()
            .try_get_matches_from(["completion", "bash"])
            .expect("completion bash should parse");
    }

    #[test]
    fn completion_zsh_is_valid() {
        build_completion_command()
            .try_get_matches_from(["completion", "zsh"])
            .expect("completion zsh should parse");
    }

    #[test]
    fn completion_install_flag_works() {
        let m = build_completion_command()
            .try_get_matches_from(["completion", "fish", "--install"])
            .expect("completion fish --install should parse");
        assert!(m.get_flag("install"));
    }
}
