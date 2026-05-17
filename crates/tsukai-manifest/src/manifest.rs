//! Core manifest types.
//!
//! This module defines the complete manifest data model: the top-level
//! [`Manifest`] struct and all nested types for commands, arguments, flags,
//! output schemas, tiers, pathways, error taxonomy, and context requirements.
//!
//! The manifest fully describes a CLI tool so that an MCP bridge can generate
//! correct tool definitions for AI agents.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Top-level manifest describing a CLI tool.
///
/// This is the canonical representation that tool authors write and that the
/// tsukai bridge reads to generate MCP tool definitions. It contains everything
/// needed to understand a tool's commands, arguments, output shapes, mutation
/// semantics, error taxonomy, and recommended workflows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// JSON Schema URI for this manifest version (e.g.
    /// `"https://tsukai.dev/manifest/v1.json"`).
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// Human-readable tool name (e.g. "mx-kv", "gh").
    pub name: String,

    /// Binary name on disk (e.g. "mx", "gh").
    pub bin: String,

    /// Tool version (the version of the CLI being described), following semver.
    /// The manifest schema version is encoded in the `$schema` URL.
    pub version: Version,

    /// One-line description of what this tool does.
    pub description: String,

    /// Base command prefix. For tools invoked as `binary subcommand` (e.g.
    /// `mx kv`), this holds the full prefix so individual command entries
    /// only need to specify the leaf name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_command: Vec<String>,

    /// Agent integration configuration (output modes, env vars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentConfig>,

    /// Runtime context requirements (network, auth, git repo, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextRequirements>,

    /// Deferred loading tiers — which commands belong to core, common, or
    /// extended tiers for the bridge's context budget management.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Tiers>,

    /// Common workflows encoded as step-by-step pathways.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pathways: Vec<Pathway>,

    /// Global error taxonomy. Individual commands reference these by `kind`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ErrorDef>,

    /// Command definitions, keyed by command name.
    ///
    /// Keys use dot notation for subcommands (e.g. `"pr.view"`,
    /// `"memory.search"`). This keeps the map flat regardless of nesting
    /// depth. The bridge reconstructs the command tree from dots when
    /// generating Tier 0 group projections.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commands: BTreeMap<String, Command>,
}

/// Agent integration configuration.
///
/// Describes how the tool can adapt its output for AI agents — which output
/// modes it supports, what flag enables machine-readable output, and any
/// environment variables the tool recognizes for agent-aware behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentConfig {
    /// Supported output modes (e.g. `["json"]`, `["json", "csv"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_modes: Vec<String>,

    /// Default flag to get machine-readable output (e.g. `"--json"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_output_flag: Option<String>,

    /// Environment variables the tool recognizes for agent behavior.
    /// Keys are var names, values are descriptions of what setting them does.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env_vars: BTreeMap<String, String>,
}

/// Runtime context requirements.
///
/// Hints about what the runtime environment must provide for this tool to
/// function. The bridge can use these to skip tools that won't work in the
/// current context (e.g. offline, no git repo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContextRequirements {
    /// Whether the tool requires network access.
    #[serde(default)]
    pub requires_network: bool,

    /// Whether the tool requires authentication to be set up first.
    #[serde(default)]
    pub requires_auth: bool,

    /// Whether the tool must be run inside a git repository.
    #[serde(default)]
    pub requires_git_repo: bool,

    /// Whether the tool requires elevated/root permissions.
    #[serde(default)]
    pub requires_elevated: bool,

    /// Free-form description of the typical environment (e.g. "any", "ci",
    /// "developer workstation").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typical_environment: Option<String>,
}

/// Deferred loading tiers.
///
/// The bridge uses these to decide which commands to include at each
/// projection tier. `core` commands are loaded when the agent engages with
/// the tool, `common` commands are available but not pre-loaded, and
/// `extended` commands are loaded only on demand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Tiers {
    /// Most important commands — loaded at Tier 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub core: Vec<String>,

    /// Frequently used commands — available but not pre-loaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common: Vec<String>,

    /// Specialized commands — loaded only on demand (Tier 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extended: Vec<String>,
}

/// A common workflow encoded as a sequence of steps.
///
/// Pathways capture expert knowledge about how to accomplish a task with the
/// tool. Instead of the agent discovering through trial and error, the
/// manifest declares the optimal sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pathway {
    /// Pathway identifier (e.g. "check-state", "create-and-push").
    pub name: String,

    /// Human-readable description of what this pathway accomplishes.
    pub description: String,

    /// Commands or conditions that must be satisfied before starting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<String>,

    /// Ordered steps to execute.
    pub steps: Vec<PathwayStep>,
}

/// A single step within a pathway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PathwayStep {
    /// Command name to invoke (must exist in the manifest's `commands` map).
    pub command: String,

    /// Named arguments to pass. Keys are argument names, values are either
    /// literal values or placeholders like `"<KEY>"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,

    /// Optional human-readable note explaining this step's purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A global error definition.
///
/// Errors are defined at the manifest level and referenced by kind from
/// individual commands. This avoids repeating the same error descriptions
/// across commands and gives agents a consistent error taxonomy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorDef {
    /// Error kind identifier (e.g. "not_found", "auth_required").
    pub kind: String,

    /// Whether this error is transient and worth retrying.
    #[serde(default)]
    pub retryable: bool,

    /// Human-readable description of the error condition.
    pub description: String,

    /// Suggested resolution (e.g. "Run 'tool auth login' first").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

/// A single CLI command definition.
///
/// Contains everything the bridge needs to generate a correct MCP tool:
/// what the command does, what it takes, what it returns, whether it's safe,
/// and what can go wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Command {
    /// Human-readable description of what this command does.
    pub description: String,

    /// Optional AI-facing description override. When present, the bridge
    /// uses this in tool definitions instead of `description`. Useful when
    /// the agent needs different context than a human.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_description: Option<String>,

    /// Whether this command changes state. `false` means read-only and
    /// safe to call speculatively.
    #[serde(default)]
    pub mutating: bool,

    /// Whether this command is irreversible or dangerous.
    /// Implies `mutating: true` — the validation layer (issue #5) enforces
    /// this invariant: `destructive: true, mutating: false` is invalid.
    #[serde(default)]
    pub destructive: bool,

    /// Whether this command requires interactive input (TTY, browser, etc.).
    #[serde(default)]
    pub interactive: bool,

    /// If `interactive` is true, an alternative invocation that works
    /// non-interactively (e.g. `"gh auth login --with-token < token_file"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_interactive_alternative: Option<String>,

    /// Positional and named arguments this command accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Arg>,

    /// Flags (options) this command accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<Flag>,

    /// Commands or conditions that must be satisfied before invoking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<String>,

    /// Schema describing the command's output shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSchema>,

    /// Error kinds this command can produce (references global error `kind`s).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// A command argument (positional or named).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Arg {
    /// Argument name.
    pub name: String,

    /// Value type (e.g. "string", "integer", "boolean").
    #[serde(rename = "type")]
    pub arg_type: String,

    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,

    /// Human-readable description.
    pub description: String,

    /// Default value when not provided, serialized as a JSON value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Allowed values for enum-style arguments.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// Validation constraints (e.g. min/max length, regex pattern).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<serde_json::Value>,
}

/// A command flag (option).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Flag {
    /// Flag name including prefix (e.g. "--json", "--id").
    pub name: String,

    /// Value type (e.g. "string", "boolean", "integer").
    #[serde(rename = "type")]
    pub flag_type: String,

    /// Whether this flag is required.
    #[serde(default)]
    pub required: bool,

    /// Human-readable description.
    pub description: String,

    /// Default value when not provided, serialized as a JSON value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// Schema describing a command's output shape.
///
/// Agents need to know the return structure before calling a command so they
/// can plan how to parse and use the result.
///
/// When `output_type` is `"object"`, `fields` describes the object's
/// properties. When `"array"`, `items` describes the schema of each
/// element (which itself can have `fields` if items are objects).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OutputSchema {
    /// Top-level output type (e.g. "object", "array").
    #[serde(rename = "type")]
    pub output_type: String,

    /// Fields within the output (used when `output_type` is `"object"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<OutputField>,

    /// For array types, the schema of each item in the array.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<OutputSchema>>,
}

/// A single field within an output schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OutputField {
    /// Field name.
    pub name: String,

    /// Field type (e.g. "string", "integer", "any").
    #[serde(rename = "type")]
    pub field_type: String,

    /// Human-readable description of this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Allowed values for enum-style fields.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_manifest() -> Manifest {
        Manifest {
            schema: Some("https://tsukai.dev/manifest/v1.json".to_string()),
            name: "mx-kv".to_string(),
            bin: "mx".to_string(),
            version: Version::new(0, 1, 0),
            description: "Key-value store for the Tsunderground".to_string(),
            base_command: vec!["mx".to_string(), "kv".to_string()],
            agent: Some(AgentConfig {
                output_modes: vec!["json".to_string()],
                default_output_flag: Some("--json".to_string()),
                env_vars: BTreeMap::from([(
                    "AGENT".to_string(),
                    "Set to 'true' for agent-optimized output".to_string(),
                )]),
            }),
            context: Some(ContextRequirements {
                requires_network: true,
                requires_auth: false,
                requires_git_repo: false,
                requires_elevated: false,
                typical_environment: Some("any".to_string()),
            }),
            tiers: Some(Tiers {
                core: vec!["get".to_string(), "set".to_string(), "list".to_string()],
                common: vec!["push".to_string(), "last".to_string(), "search".to_string()],
                extended: vec![
                    "pop".to_string(),
                    "reset".to_string(),
                    "random".to_string(),
                    "count".to_string(),
                ],
            }),
            pathways: vec![Pathway {
                name: "check-state".to_string(),
                description: "See what keys exist and get a value".to_string(),
                prerequisites: vec![],
                steps: vec![
                    PathwayStep {
                        command: "keys".to_string(),
                        args: BTreeMap::new(),
                        note: Some("List all defined keys with types".to_string()),
                    },
                    PathwayStep {
                        command: "get".to_string(),
                        args: BTreeMap::from([("key".to_string(), "<KEY>".to_string())]),
                        note: Some("Get current value".to_string()),
                    },
                ],
            }],
            errors: vec![
                ErrorDef {
                    kind: "not_found".to_string(),
                    retryable: false,
                    description: "Resource does not exist".to_string(),
                    resolution: None,
                },
                ErrorDef {
                    kind: "auth_required".to_string(),
                    retryable: false,
                    description: "Authentication needed".to_string(),
                    resolution: Some("Run 'mx auth login' first".to_string()),
                },
                ErrorDef {
                    kind: "connection".to_string(),
                    retryable: true,
                    description: "Network connection failed".to_string(),
                    resolution: None,
                },
            ],
            commands: BTreeMap::from([(
                "get".to_string(),
                Command {
                    description: "Get the current value of a key".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![Arg {
                        name: "key".to_string(),
                        arg_type: "string".to_string(),
                        required: true,
                        description: "Key name".to_string(),
                        default: None,
                        enum_values: None,
                        constraints: None,
                    }],
                    flags: vec![
                        Flag {
                            name: "--id".to_string(),
                            flag_type: "string".to_string(),
                            required: false,
                            description: "Entry ID or range".to_string(),
                            default: None,
                        },
                        Flag {
                            name: "--json".to_string(),
                            flag_type: "boolean".to_string(),
                            required: false,
                            description: "Output as JSON".to_string(),
                            default: None,
                        },
                    ],
                    prerequisites: vec![],
                    output: Some(OutputSchema {
                        output_type: "object".to_string(),
                        fields: vec![
                            OutputField {
                                name: "key".to_string(),
                                field_type: "string".to_string(),
                                description: None,
                                enum_values: None,
                            },
                            OutputField {
                                name: "type".to_string(),
                                field_type: "string".to_string(),
                                description: None,
                                enum_values: Some(vec![
                                    "string".to_string(),
                                    "counter".to_string(),
                                    "list".to_string(),
                                    "history".to_string(),
                                    "state".to_string(),
                                ]),
                            },
                            OutputField {
                                name: "value".to_string(),
                                field_type: "any".to_string(),
                                description: Some("Current value".to_string()),
                                enum_values: None,
                            },
                        ],
                        items: None,
                    }),
                    errors: vec!["not_found".to_string(), "connection".to_string()],
                },
            )]),
        }
    }

    #[test]
    fn round_trip_serialization() {
        let manifest = minimal_manifest();
        let json = serde_json::to_string_pretty(&manifest).expect("serialize");
        let deserialized: Manifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn minimal_manifest_omits_empty_fields() {
        let manifest = Manifest {
            schema: None,
            name: "simple".to_string(),
            bin: "simple".to_string(),
            version: Version::new(1, 0, 0),
            description: "A simple tool".to_string(),
            base_command: vec![],
            agent: None,
            context: None,
            tiers: None,
            pathways: vec![],
            errors: vec![],
            commands: BTreeMap::new(),
        };

        let json = serde_json::to_string(&manifest).expect("serialize");

        // Optional/empty fields should not appear in output
        assert!(!json.contains("base_command"));
        assert!(!json.contains("agent"));
        assert!(!json.contains("context"));
        assert!(!json.contains("tiers"));
        assert!(!json.contains("pathways"));
        assert!(!json.contains("errors"));
        assert!(!json.contains("commands"));
    }

    #[test]
    fn defaults_applied_on_deserialize() {
        let json = r#"{
            "name": "test",
            "bin": "test",
            "version": "0.1.0",
            "description": "test tool"
        }"#;

        let manifest: Manifest = serde_json::from_str(json).expect("deserialize");
        assert!(manifest.base_command.is_empty());
        assert!(manifest.agent.is_none());
        assert!(manifest.context.is_none());
        assert!(manifest.tiers.is_none());
        assert!(manifest.pathways.is_empty());
        assert!(manifest.errors.is_empty());
        assert!(manifest.commands.is_empty());
    }

    #[test]
    fn command_defaults() {
        let json = r#"{
            "description": "Do something"
        }"#;

        let cmd: Command = serde_json::from_str(json).expect("deserialize");
        assert!(!cmd.mutating);
        assert!(!cmd.destructive);
        assert!(!cmd.interactive);
        assert!(cmd.agent_description.is_none());
        assert!(cmd.non_interactive_alternative.is_none());
        assert!(cmd.args.is_empty());
        assert!(cmd.flags.is_empty());
        assert!(cmd.prerequisites.is_empty());
        assert!(cmd.output.is_none());
        assert!(cmd.errors.is_empty());
    }

    /// Verify that the exact JSON from ARCHITECTURE.md (the `commands.get`
    /// example) deserializes into the Rust types without modification.
    /// This catches any mismatch between what the spec says and what the
    /// types accept.
    #[test]
    fn deserialize_architecture_example() {
        // Exact JSON from ARCHITECTURE.md lines 153-187 — the commands.get example.
        let json = r#"{
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
}"#;

        // Wrap in a minimal valid manifest
        let full_json = format!(
            r#"{{
  "$schema": "https://tsukai.dev/manifest/v1.json",
  "name": "mx-kv",
  "bin": "mx",
  "version": "0.1.0",
  "description": "Key-value store",
  {commands}
}}"#,
            commands = &json.trim()[1..json.trim().len() - 1]
        );

        let manifest: Manifest =
            serde_json::from_str(&full_json).expect("architecture doc JSON must deserialize");

        let cmd = manifest
            .commands
            .get("get")
            .expect("'get' command must exist");

        assert_eq!(cmd.description, "Get the current value of a key");
        assert_eq!(
            cmd.agent_description.as_deref(),
            Some("Optional override for AI-facing description")
        );
        assert!(!cmd.mutating);
        assert!(!cmd.destructive);
        assert!(!cmd.interactive);
        assert!(cmd.non_interactive_alternative.is_none());
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0].name, "key");
        assert!(cmd.args[0].required);
        assert_eq!(cmd.flags.len(), 2);
        assert_eq!(cmd.prerequisites.len(), 0);

        let output = cmd.output.as_ref().expect("output must exist");
        assert_eq!(output.output_type, "object");
        assert_eq!(output.fields.len(), 3);

        // The doc uses "enum" — verify our serde rename handles it
        let type_field = &output.fields[1];
        assert_eq!(type_field.name, "type");
        assert_eq!(
            type_field.enum_values,
            Some(vec![
                "string".to_string(),
                "counter".to_string(),
                "list".to_string(),
                "history".to_string(),
                "state".to_string(),
            ])
        );

        assert_eq!(cmd.errors, vec!["not_found", "connection"]);
    }
}
