//! Tier projection logic.
//!
//! Generates compact, tiered representations of a manifest for agent
//! consumption. The bridge uses these projections to manage context window
//! budgets:
//!
//! - **Tier 0 (Discovery):** Tool name, description, command groups (~150-300 tokens)
//! - **Tier 1 (Core Commands):** Key commands with args, return types, mutation flags (~600 tokens)
//! - **Tier 2 (Extended):** Full command details, loaded on demand

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::{Command, Manifest, OutputSchema};

// ---------------------------------------------------------------------------
// Tier 0 — Discovery
// ---------------------------------------------------------------------------

/// High-level tool overview for discovery (~150-300 tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier0 {
    /// Tool name from the manifest.
    pub tool: String,
    /// One-line tool description.
    pub description: String,
    /// Command groups inferred from dot-notation keys.
    pub groups: BTreeMap<String, CommandGroupSummary>,
    /// Top-level commands (no dots in the key).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    /// Commands where `interactive == true`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactive_commands: Vec<String>,
    /// The `agent.default_output_flag` value, if any.
    pub agent_output: Option<String>,
    /// Pathway names only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pathways: Vec<String>,
}

/// Summary of commands within a dot-notation group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandGroupSummary {
    /// Leaf command names within this group.
    pub commands: Vec<String>,
    /// Group description. Will be populated when the manifest schema gains
    /// explicit group metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Tier 1 — Core Commands
// ---------------------------------------------------------------------------

/// Core command summaries for engaged usage (~600 tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier1 {
    /// Tool name from the manifest.
    pub tool: String,
    /// Only commands listed in `tiers.core`.
    pub commands: BTreeMap<String, CoreCommandSummary>,
    /// Pathway name to compressed step description.
    pub pathways: BTreeMap<String, String>,
    /// Error kinds with "(retryable)" suffix where applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// Compact representation of a core command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreCommandSummary {
    /// Human-readable args string (e.g. `"<KEY> [--id ID]"`).
    pub args: String,
    /// Human-readable return shape (e.g. `"{key, type, value}"`).
    pub returns: String,
    /// `true` when the command is neither mutating nor destructive.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub readonly: bool,
    /// Whether the command mutates state.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mutating: bool,
    /// Whether the command is destructive.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
}

// ---------------------------------------------------------------------------
// Tier 2 — Extended Per-Command
// ---------------------------------------------------------------------------

/// Full detail for a single command, loaded on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Command {
    /// Command name.
    pub command: String,
    /// Full description (prefers `agent_description` when present).
    pub description: String,
    /// Whether the command mutates state.
    pub mutating: bool,
    /// Whether the command is destructive.
    pub destructive: bool,
    /// Whether the command requires interactive input.
    pub interactive: bool,
    /// Non-interactive alternative invocation, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_interactive_alternative: Option<String>,
    /// Commands or conditions that must be satisfied before invoking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<String>,
    /// Positional / named arguments.
    pub args: Vec<Tier2Arg>,
    /// Flags / options.
    pub flags: Vec<Tier2Flag>,
    /// Output schema (reuses the manifest type directly).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSchema>,
    /// Resolved error details from the global taxonomy.
    pub errors: Vec<ErrorDetail>,
}

/// Full argument detail for Tier 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Arg {
    /// Argument name.
    pub name: String,
    /// Value type.
    #[serde(rename = "type")]
    pub arg_type: String,
    /// Whether this argument is required.
    pub required: bool,
    /// Human-readable description.
    pub description: String,
    /// Default value, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Allowed enum values, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// Validation constraints, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<serde_json::Value>,
}

/// Full flag detail for Tier 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Flag {
    /// Flag name (e.g. `"--json"`).
    pub name: String,
    /// Value type.
    #[serde(rename = "type")]
    pub flag_type: String,
    /// Whether this flag is required.
    pub required: bool,
    /// Human-readable description.
    pub description: String,
    /// Default value, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// Resolved error detail from the global taxonomy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// Error kind identifier.
    pub kind: String,
    /// Whether this error is transient.
    pub retryable: bool,
    /// Human-readable description.
    pub description: String,
    /// Suggested resolution, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

// ---------------------------------------------------------------------------
// Token estimation
// ---------------------------------------------------------------------------

/// Rough token estimate (~4 chars per token).
pub fn estimate_tokens(s: &str) -> usize {
    s.len() / 4
}

// ---------------------------------------------------------------------------
// Projection functions
// ---------------------------------------------------------------------------

/// Project a manifest to Tier 0 (discovery).
pub fn project_tier0(manifest: &Manifest) -> Tier0 {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut top_level: Vec<String> = Vec::new();
    let mut interactive_commands: Vec<String> = Vec::new();

    for (key, cmd) in &manifest.commands {
        if let Some((prefix, leaf)) = key.split_once('.') {
            groups.entry(prefix.to_string()).or_default().push(leaf.to_string());
        } else {
            top_level.push(key.clone());
        }

        if cmd.interactive {
            interactive_commands.push(key.clone());
        }
    }

    let groups = groups
        .into_iter()
        .map(|(name, commands)| {
            (
                name,
                CommandGroupSummary {
                    commands,
                    description: None,
                },
            )
        })
        .collect();

    let agent_output = manifest
        .agent
        .as_ref()
        .and_then(|a| a.default_output_flag.clone());

    let pathways = manifest.pathways.iter().map(|p| p.name.clone()).collect();

    Tier0 {
        tool: manifest.name.clone(),
        description: manifest.description.clone(),
        groups,
        commands: top_level,
        interactive_commands,
        agent_output,
        pathways,
    }
}

/// Project a manifest to Tier 1 (core commands).
pub fn project_tier1(manifest: &Manifest) -> Tier1 {
    let core_names: Vec<&str> = manifest
        .tiers
        .as_ref()
        .map(|t| t.core.iter().map(String::as_str).collect())
        .unwrap_or_default();

    let mut commands = BTreeMap::new();
    for name in &core_names {
        if let Some(cmd) = manifest.commands.get(*name) {
            commands.insert((*name).to_string(), summarize_command(cmd));
        }
    }

    // Compress each pathway into a single string of the form
    // "cmd1 ARG1 ARG2 -> cmd2 ARG1". Note: args appear in BTreeMap key
    // order (alphabetical), not insertion order, because PathwayStep.args
    // is a BTreeMap.
    let pathways = manifest
        .pathways
        .iter()
        .map(|p| {
            let compressed = p
                .steps
                .iter()
                .map(|step| {
                    let mut s = step.command.clone();
                    for v in step.args.values() {
                        s.push(' ');
                        s.push_str(v);
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            (p.name.clone(), compressed)
        })
        .collect();

    let errors = manifest
        .errors
        .iter()
        .map(|e| {
            if e.retryable {
                format!("{} (retryable)", e.kind)
            } else {
                e.kind.clone()
            }
        })
        .collect();

    Tier1 {
        tool: manifest.name.clone(),
        commands,
        pathways,
        errors,
    }
}

/// Project a single command to Tier 2 (full detail).
///
/// Error kinds listed on the command are resolved against the manifest's
/// global error taxonomy. If a kind is not found in the taxonomy, a fallback
/// entry is produced with `retryable: false` and an empty description.
///
/// Returns `None` if the command does not exist in the manifest.
pub fn project_tier2_command(manifest: &Manifest, command: &str) -> Option<Tier2Command> {
    let cmd = manifest.commands.get(command)?;

    let description = cmd
        .agent_description
        .as_deref()
        .unwrap_or(&cmd.description)
        .to_string();

    let args = cmd
        .args
        .iter()
        .map(|a| Tier2Arg {
            name: a.name.clone(),
            arg_type: a.arg_type.clone(),
            required: a.required,
            description: a.description.clone(),
            default: a.default.clone(),
            enum_values: a.enum_values.clone(),
            constraints: a.constraints.clone(),
        })
        .collect();

    let flags = cmd
        .flags
        .iter()
        .map(|f| Tier2Flag {
            name: f.name.clone(),
            flag_type: f.flag_type.clone(),
            required: f.required,
            description: f.description.clone(),
            default: f.default.clone(),
        })
        .collect();

    // Resolve error kinds from the global taxonomy.
    let error_map: BTreeMap<&str, _> = manifest
        .errors
        .iter()
        .map(|e| (e.kind.as_str(), e))
        .collect();

    let errors = cmd
        .errors
        .iter()
        .map(|kind| {
            match error_map.get(kind.as_str()) {
                Some(e) => ErrorDetail {
                    kind: e.kind.clone(),
                    retryable: e.retryable,
                    description: e.description.clone(),
                    resolution: e.resolution.clone(),
                },
                None => ErrorDetail {
                    kind: kind.clone(),
                    retryable: false,
                    description: String::new(),
                    resolution: None,
                },
            }
        })
        .collect();

    Some(Tier2Command {
        command: command.to_string(),
        description,
        mutating: cmd.mutating,
        destructive: cmd.destructive,
        interactive: cmd.interactive,
        non_interactive_alternative: cmd.non_interactive_alternative.clone(),
        prerequisites: cmd.prerequisites.clone(),
        args,
        flags,
        output: cmd.output.clone(),
        errors,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a compact `CoreCommandSummary` from a full `Command`.
fn summarize_command(cmd: &Command) -> CoreCommandSummary {
    CoreCommandSummary {
        args: summarize_args(cmd),
        returns: summarize_returns(cmd),
        readonly: !cmd.mutating && !cmd.destructive,
        mutating: cmd.mutating,
        destructive: cmd.destructive,
    }
}

/// Format human-readable args string.
///
/// Required args -> `<NAME>`, optional args -> `[NAME]`,
/// flags -> `[--name TYPE]` (boolean flags omit the type).
fn summarize_args(cmd: &Command) -> String {
    let mut parts: Vec<String> = Vec::new();

    for arg in &cmd.args {
        let upper = arg.name.to_uppercase();
        if arg.required {
            parts.push(format!("<{upper}>"));
        } else {
            parts.push(format!("[{upper}]"));
        }
    }

    for flag in &cmd.flags {
        let name = &flag.name;
        if flag.flag_type == "boolean" {
            parts.push(format!("[{name}]"));
        } else {
            let type_upper = flag.flag_type.to_uppercase();
            parts.push(format!("[{name} {type_upper}]"));
        }
    }

    parts.join(" ")
}

/// Format human-readable return shape.
///
/// Object -> `{field1, field2, ...}`.
/// Array of objects -> `[{field1, field2, ...}]`.
/// No output -> `"(none)"`.
fn summarize_returns(cmd: &Command) -> String {
    match &cmd.output {
        None => "(none)".to_string(),
        Some(schema) => format_output_schema(schema),
    }
}

/// Recursively format an `OutputSchema` into a compact string.
fn format_output_schema(schema: &OutputSchema) -> String {
    match schema.output_type.as_str() {
        "object" => {
            let fields: Vec<&str> = schema.fields.iter().map(|f| f.name.as_str()).collect();
            format!("{{{}}}", fields.join(", "))
        }
        "array" => {
            if let Some(items) = &schema.items {
                format!("[{}]", format_output_schema(items))
            } else {
                "[]".to_string()
            }
        }
        other => other.to_string(),
    }
}
