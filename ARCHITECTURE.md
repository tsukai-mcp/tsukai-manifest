# tsukai-manifest — Architecture

使い (tsukai) — the messenger. A structured CLI manifest format designed for AI agent consumption via MCP.

## The Problem

AI agents interact with CLI tools through trial and error. An agent discovers a command exists, tries to call it, and either:
- Gets it wrong (`mx memory get` when the command is `mx memory show`)
- Hangs on interactive input (`gh auth login` waiting for a browser)
- Doesn't know the output shape and can't parse the result
- Burns 3-4 exploratory calls to figure out the happy path

Meanwhile, existing approaches fail at scale:
- `--help` text is human-readable, not machine-parseable
- Hand-written MCP servers per tool don't scale
- Existing CLI spec formats (clispec.dev, usage spec) were designed for shells, not agents

## The Solution: Two-Layer Architecture

### Layer 1: The Full Manifest (tool-side, human-writable)

The complete description of a CLI tool. Everything the bridge needs to generate correct MCP tools. Written by tool authors, exposed via a `tool manifest` command.

**Format:** JSON (canonical), KDL (optional authoring with transpiler)

**Contains:**
- Complete command tree with nested subcommands
- Argument types, descriptions, defaults, constraints
- Output schemas (what does each command RETURN?)
- Mutation markers (readonly / mutating / destructive)
- Interactive command detection with non-interactive alternatives
- Error taxonomy with retry hints
- Common pathways (tldr for machines)
- Prerequisite chains
- Context hints (requires git repo, network, auth, permissions)
- Deferred loading tiers (core / common / extended)
- Agent-specific output mode detection (`--json`, `AGENT` env var)

### Layer 2: The Agent Projection (bridge-side, machine-consumed)

Compact, tiered representations generated FROM the full manifest by the tsukai MCP bridge. Optimized for token efficiency.

**Tier 0 — Discovery (~150 tokens per tool):**
Tool name, description, command groups with one-line descriptions. Loaded for every registered tool at boot. The agent knows what's available without details.

**Tier 1 — Core Commands (~600 tokens per tool):**
The most important commands (defined by `tiers.core` in the manifest) with args, return types, and mutation flags. Promoted into context when the agent decides to use the tool.

**Tier 2 — Extended (on-demand):**
Full command details including all flags, output field descriptions, examples. Loaded per-command when the agent needs specifics.

This solves the fundamental tension: the manifest must be **complete** enough for the bridge to generate correct MCP tools, but **compact** enough that the agent's context window isn't consumed by tool definitions alone.

## Manifest Schema

### Top-Level Structure

```json
{
  "$schema": "https://tsukai.yaoyorozu.industries/manifest/v1.json",
  "name": "tool-name",
  "bin": "binary-name",
  "version": "0.1.0",
  "description": "What this tool does",
  "base_command": ["binary", "subcommand"],

  "agent": { },
  "context": { },
  "tiers": { },
  "pathways": [ ],
  "errors": [ ],
  "commands": { }
}
```

### `agent` — Agent Integration

```json
{
  "agent": {
    "output_modes": ["json"],
    "default_output_flag": "--json",
    "env_vars": {
      "AGENT": "Set to 'true' for agent-optimized output"
    }
  }
}
```

### `context` — Runtime Requirements

```json
{
  "context": {
    "requires_network": true,
    "requires_auth": false,
    "requires_git_repo": false,
    "requires_elevated": false,
    "typical_environment": "any"
  }
}
```

### `tiers` — Deferred Loading

```json
{
  "tiers": {
    "core": ["get", "set", "list"],
    "common": ["push", "last", "search"],
    "extended": ["pop", "reset", "random", "count"]
  }
}
```

The bridge loads Tier 0 (discovery) for all tools, promotes to Tier 1 (core commands) when the agent engages with a tool, and serves Tier 2 (extended) on-demand.

### `pathways` — Common Workflows (tldr for machines)

```json
{
  "pathways": [
    {
      "name": "check-state",
      "description": "See what keys exist and get a value",
      "prerequisites": [],
      "steps": [
        {"command": "keys", "note": "List all defined keys with types"},
        {"command": "get", "args": {"key": "<KEY>"}, "note": "Get current value"}
      ]
    }
  ]
}
```

Pathways encode expert knowledge. Instead of the agent discovering through trial and error that `gh pr view <number> --json state,mergeable,reviewDecision,statusCheckRollup` is the way to check PR status, the manifest declares it as a pathway. One lookup replaces 3-4 exploratory calls.

### `errors` — Error Taxonomy

```json
{
  "errors": [
    {"kind": "not_found", "retryable": false, "description": "Resource does not exist"},
    {"kind": "auth_required", "retryable": false, "description": "Authentication needed", "resolution": "Run 'tool auth login' first"},
    {"kind": "connection", "retryable": true, "description": "Network connection failed"}
  ]
}
```

### `commands` — Command Definitions

**Command Naming Convention:**
Keys use dot notation for subcommands: `"pr.view"`, `"memory.search"`, `"auth.login.web"`. The map stays flat regardless of nesting depth. The bridge reconstructs command groups from dot-separated prefixes when generating Tier 0 projections.

```json
{
  "commands": {
    "get": {
      "description": "Get the current value of a key",
      "agent_description": "Optional override for AI-facing description",
      "mutating": false,
      "destructive": false,
      "interactive": false,
      "non_interactive_alternative": null,

      "args": [
        {"name": "key", "type": "string", "required": true, "description": "Key name"}
      ],

      "flags": [
        {"name": "--id", "type": "string", "required": false, "description": "Entry ID or range"},
        {"name": "--json", "type": "boolean", "required": false, "description": "Output as JSON"}
      ],

      "prerequisites": [],

      "output": {
        "type": "object",
        "fields": [
          {"name": "key", "type": "string"},
          {"name": "type", "type": "string", "enum": ["string", "counter", "list", "history", "state"]},
          {"name": "value", "type": "any", "description": "Current value"}
        ]
      },

      "errors": ["not_found", "connection"]
    }
  }
}
```

#### Mutation Markers

Three levels, not two:

| Marker | Meaning | Example |
|--------|---------|---------|
| `mutating: false` | Read-only, safe to call speculatively | `gh pr view`, `mx kv get` |
| `mutating: true` | Changes state, but reversible/additive | `gh pr create`, `mx kv push` |
| `destructive: true` | Irreversible or dangerous | `gh repo delete`, `mx kv reset` |

#### Interactive Detection

```json
{
  "interactive": true,
  "non_interactive_alternative": "gh auth login --with-token < token_file"
}
```

If `interactive: true` and no `non_interactive_alternative` exists, the agent should NOT call this command. If an alternative exists, the agent uses that instead.

## Agent Projection Examples

### Tier 0 for `gh` (~300 tokens)

```json
{
  "tool": "gh",
  "description": "GitHub CLI",
  "groups": {
    "auth": {"description": "Authentication", "commands": ["login", "logout", "status", "token"]},
    "issue": {"description": "Issues", "commands": ["create", "list", "view", "close", "edit"]},
    "pr": {"description": "Pull requests", "commands": ["create", "list", "view", "merge", "checkout"]},
    "repo": {"description": "Repositories", "commands": ["clone", "create", "fork", "view"]},
    "api": {"description": "Raw API calls", "commands": ["<endpoint>"]}
  },
  "interactive_commands": ["auth login"],
  "agent_output": "--json <fields> on most commands",
  "pathways": ["check-pr-status", "create-issue", "authenticate"]
}
```

### Tier 1 for `mx kv` (~600 tokens)

```json
{
  "tool": "mx-kv",
  "commands": {
    "get": {"args": "<KEY> [--id STRING]", "returns": "{key, type, value}", "readonly": true},
    "set": {"args": "<KEY> [VALUE] [--memory STRING]", "returns": "{key, value, previous}", "mutating": true},
    "keys": {"args": "", "returns": "[{key, type}]", "readonly": true},
    "search": {"args": "<KEY> [QUERY] [--where STRING]", "returns": "[{id, value, timestamp}]", "readonly": true}
  },
  "pathways": {
    "check-state": "keys -> get <KEY>",
    "track-history": "push <KEY> <VALUE> -> last <KEY> 5"
  },
  "errors": ["not_found", "type_mismatch", "connection (retryable)"]
}
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Base format | Custom | Neither clispec nor usage spec has >50% of what agents need |
| Serialization | JSON canonical, KDL optional | Universal parsing, schema validation, LLM-native |
| Token efficiency | Tiered projection (0/1/2) | Full manifest is reference truth; agents see projections sized to need |
| Output schemas | Per-command | Agent MUST know return shape before calling |
| Mutation markers | Tristate | `destructive` is distinct from `mutating` |
| Interactive detection | Boolean + alternative | Agent needs to know AND needs a workaround |
| Pathways | First-class | Highest-value novel feature; encodes expert knowledge |
| Error taxonomy | Global + per-command | Common errors at root, overrides where needed |
| Prerequisites | Per-command and per-pathway | Machine-readable dependency chains |
| Deferred loading | Tiers in manifest | Bridge decides injection based on tier config |

## Ecosystem

| Component | Purpose |
|-----------|---------|
| **tsukai** | MCP bridge server — reads manifests, generates tools, hot-reloads |
| **tsukai-manifest** | This spec — format definition, JSON schema, validation |
| **tsukai-derive** | Rust derive macros — generate manifests from clap definitions |

## Prior Art

- **clispec.dev** — Right instincts about agent needs (output schemas, mutation markers, error taxonomy). v0.1, no tooling, no ecosystem. We take the agent-awareness concepts.
- **usage spec (jdx)** — Mature implementation (powers mise). Excellent command tree modeling, shell completions, docs generation. Designed for shells, not agents. We take the structural maturity.
- **glab mcp serve** — GitLab CLI self-introspects for MCP. Closest existing implementation but glab-specific, not generalizable.
- **MCP `notifications/tools/list_changed`** — The hot-reload protocol primitive already exists. We use it.
- **`AGENT` env var convention** — Emerging standard for AI-aware CLI output. Adopted by Goose, Amp, Bun.

---

*The manifest carries the full truth. The projection carries what the agent needs. The bridge does the translation. The messenger delivers both.*
