//! Integration tests for tier projection logic.

use std::collections::BTreeMap;

use semver::Version;
use tsukai_manifest::{
    AgentConfig, Arg, Command, ErrorDef, Flag, Manifest, OutputField, OutputSchema, Pathway,
    PathwayStep, Tiers, estimate_tokens, project_tier0, project_tier1, project_tier2_command,
};

/// Build a rich test manifest with grouped commands, tiers, pathways, errors,
/// and interactive commands.
fn rich_manifest() -> Manifest {
    Manifest {
        schema: Some("https://tsukai.yaoyorozu.industries/manifest/v1.json".to_string()),
        name: "test-tool".to_string(),
        bin: "tt".to_string(),
        version: Version::new(1, 0, 0),
        description: "A test tool for projection".to_string(),
        base_command: vec!["tt".to_string()],
        agent: Some(AgentConfig {
            output_modes: vec!["json".to_string()],
            default_output_flag: Some("--json".to_string()),
            env_vars: BTreeMap::new(),
        }),
        context: None,
        tiers: Some(Tiers {
            core: vec!["get".to_string(), "set".to_string(), "pr.view".to_string()],
            common: vec!["pr.create".to_string(), "issue.list".to_string()],
            extended: vec!["pr.merge".to_string(), "login".to_string()],
        }),
        pathways: vec![
            Pathway {
                name: "check-state".to_string(),
                description: "Check current state".to_string(),
                prerequisites: vec![],
                steps: vec![
                    PathwayStep {
                        command: "list".to_string(),
                        args: BTreeMap::new(),
                        note: None,
                    },
                    PathwayStep {
                        command: "get".to_string(),
                        args: BTreeMap::from([("key".to_string(), "<KEY>".to_string())]),
                        note: None,
                    },
                ],
            },
            Pathway {
                name: "create-pr".to_string(),
                description: "Create a pull request".to_string(),
                prerequisites: vec!["auth".to_string()],
                steps: vec![
                    PathwayStep {
                        command: "pr.create".to_string(),
                        args: BTreeMap::from([("title".to_string(), "<TITLE>".to_string())]),
                        note: None,
                    },
                    PathwayStep {
                        command: "pr.view".to_string(),
                        args: BTreeMap::from([("number".to_string(), "<NUMBER>".to_string())]),
                        note: None,
                    },
                ],
            },
        ],
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
                resolution: Some("Run 'tt auth login' first".to_string()),
            },
            ErrorDef {
                kind: "connection".to_string(),
                retryable: true,
                description: "Network connection failed".to_string(),
                resolution: None,
            },
        ],
        commands: BTreeMap::from([
            (
                "get".to_string(),
                Command {
                    description: "Get a value by key".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![Arg {
                        name: "key".to_string(),
                        arg_type: "string".to_string(),
                        required: true,
                        description: "The key to look up".to_string(),
                        default: None,
                        enum_values: None,
                        constraints: None,
                    }],
                    flags: vec![Flag {
                        name: "--id".to_string(),
                        flag_type: "string".to_string(),
                        required: false,
                        description: "Entry ID".to_string(),
                        default: None,
                    }],
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
                                enum_values: None,
                            },
                            OutputField {
                                name: "value".to_string(),
                                field_type: "any".to_string(),
                                description: None,
                                enum_values: None,
                            },
                        ],
                        items: None,
                    }),
                    errors: vec!["not_found".to_string(), "connection".to_string()],
                },
            ),
            (
                "set".to_string(),
                Command {
                    description: "Set a key to a value".to_string(),
                    agent_description: Some("Store a value under a key".to_string()),
                    mutating: true,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![
                        Arg {
                            name: "key".to_string(),
                            arg_type: "string".to_string(),
                            required: true,
                            description: "Key name".to_string(),
                            default: None,
                            enum_values: None,
                            constraints: None,
                        },
                        Arg {
                            name: "value".to_string(),
                            arg_type: "string".to_string(),
                            required: false,
                            description: "Value to set".to_string(),
                            default: None,
                            enum_values: None,
                            constraints: None,
                        },
                    ],
                    flags: vec![],
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
                                name: "value".to_string(),
                                field_type: "any".to_string(),
                                description: None,
                                enum_values: None,
                            },
                        ],
                        items: None,
                    }),
                    errors: vec!["connection".to_string()],
                },
            ),
            (
                "list".to_string(),
                Command {
                    description: "List all keys".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![],
                    flags: vec![],
                    prerequisites: vec![],
                    output: Some(OutputSchema {
                        output_type: "array".to_string(),
                        fields: vec![],
                        items: Some(Box::new(OutputSchema {
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
                                    enum_values: None,
                                },
                            ],
                            items: None,
                        })),
                    }),
                    errors: vec![],
                },
            ),
            (
                "login".to_string(),
                Command {
                    description: "Authenticate with the service".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: true,
                    non_interactive_alternative: Some("tt login --token <TOKEN>".to_string()),
                    args: vec![],
                    flags: vec![],
                    prerequisites: vec![],
                    output: None,
                    errors: vec!["auth_required".to_string()],
                },
            ),
            (
                "pr.view".to_string(),
                Command {
                    description: "View a pull request".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![Arg {
                        name: "number".to_string(),
                        arg_type: "integer".to_string(),
                        required: true,
                        description: "PR number".to_string(),
                        default: None,
                        enum_values: None,
                        constraints: None,
                    }],
                    flags: vec![Flag {
                        name: "--json".to_string(),
                        flag_type: "boolean".to_string(),
                        required: false,
                        description: "Output as JSON".to_string(),
                        default: None,
                    }],
                    prerequisites: vec![],
                    output: Some(OutputSchema {
                        output_type: "object".to_string(),
                        fields: vec![
                            OutputField {
                                name: "number".to_string(),
                                field_type: "integer".to_string(),
                                description: None,
                                enum_values: None,
                            },
                            OutputField {
                                name: "title".to_string(),
                                field_type: "string".to_string(),
                                description: None,
                                enum_values: None,
                            },
                            OutputField {
                                name: "state".to_string(),
                                field_type: "string".to_string(),
                                description: None,
                                enum_values: Some(vec![
                                    "open".to_string(),
                                    "closed".to_string(),
                                    "merged".to_string(),
                                ]),
                            },
                        ],
                        items: None,
                    }),
                    errors: vec!["not_found".to_string()],
                },
            ),
            (
                "pr.create".to_string(),
                Command {
                    description: "Create a pull request".to_string(),
                    agent_description: None,
                    mutating: true,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![Arg {
                        name: "title".to_string(),
                        arg_type: "string".to_string(),
                        required: true,
                        description: "PR title".to_string(),
                        default: None,
                        enum_values: None,
                        constraints: None,
                    }],
                    flags: vec![],
                    prerequisites: vec![],
                    output: Some(OutputSchema {
                        output_type: "object".to_string(),
                        fields: vec![OutputField {
                            name: "url".to_string(),
                            field_type: "string".to_string(),
                            description: None,
                            enum_values: None,
                        }],
                        items: None,
                    }),
                    errors: vec!["auth_required".to_string()],
                },
            ),
            (
                "pr.merge".to_string(),
                Command {
                    description: "Merge a pull request".to_string(),
                    agent_description: None,
                    mutating: true,
                    destructive: true,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![Arg {
                        name: "number".to_string(),
                        arg_type: "integer".to_string(),
                        required: true,
                        description: "PR number".to_string(),
                        default: None,
                        enum_values: None,
                        constraints: None,
                    }],
                    flags: vec![],
                    prerequisites: vec![],
                    output: None,
                    errors: vec!["not_found".to_string(), "auth_required".to_string()],
                },
            ),
            (
                "issue.list".to_string(),
                Command {
                    description: "List issues".to_string(),
                    agent_description: None,
                    mutating: false,
                    destructive: false,
                    interactive: false,
                    non_interactive_alternative: None,
                    args: vec![],
                    flags: vec![Flag {
                        name: "--state".to_string(),
                        flag_type: "string".to_string(),
                        required: false,
                        description: "Filter by state".to_string(),
                        default: None,
                    }],
                    prerequisites: vec![],
                    output: Some(OutputSchema {
                        output_type: "array".to_string(),
                        fields: vec![],
                        items: Some(Box::new(OutputSchema {
                            output_type: "object".to_string(),
                            fields: vec![
                                OutputField {
                                    name: "number".to_string(),
                                    field_type: "integer".to_string(),
                                    description: None,
                                    enum_values: None,
                                },
                                OutputField {
                                    name: "title".to_string(),
                                    field_type: "string".to_string(),
                                    description: None,
                                    enum_values: None,
                                },
                            ],
                            items: None,
                        })),
                    }),
                    errors: vec![],
                },
            ),
        ]),
    }
}

// =========================================================================
// Tier 0 tests
// =========================================================================

#[test]
fn tier0_completeness() {
    let manifest = rich_manifest();
    let t0 = project_tier0(&manifest);

    assert_eq!(t0.tool, "test-tool");
    assert_eq!(t0.description, "A test tool for projection");

    // Groups: pr and issue
    assert!(t0.groups.contains_key("pr"));
    assert!(t0.groups.contains_key("issue"));

    // Interactive commands
    assert!(t0.interactive_commands.contains(&"login".to_string()));

    // Pathways listed
    assert_eq!(t0.pathways, vec!["check-state", "create-pr"]);

    // Agent output
    assert_eq!(t0.agent_output.as_deref(), Some("--json"));
}

#[test]
fn tier0_grouping() {
    let manifest = rich_manifest();
    let t0 = project_tier0(&manifest);

    // pr group should contain view, create, merge
    let pr_group = t0.groups.get("pr").expect("pr group must exist");
    assert!(pr_group.commands.contains(&"view".to_string()));
    assert!(pr_group.commands.contains(&"create".to_string()));
    assert!(pr_group.commands.contains(&"merge".to_string()));

    // issue group should contain list
    let issue_group = t0.groups.get("issue").expect("issue group must exist");
    assert_eq!(issue_group.commands, vec!["list".to_string()]);

    // Top-level commands (no dots): get, set, list, login
    assert!(t0.commands.contains(&"get".to_string()));
    assert!(t0.commands.contains(&"set".to_string()));
    assert!(t0.commands.contains(&"list".to_string()));
    assert!(t0.commands.contains(&"login".to_string()));

    // Grouped commands should NOT appear in top-level
    assert!(!t0.commands.contains(&"pr.view".to_string()));
    assert!(!t0.commands.contains(&"issue.list".to_string()));
}

// =========================================================================
// Tier 1 tests
// =========================================================================

#[test]
fn tier1_core_filtering() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    // Only core commands: get, set, pr.view
    assert!(t1.commands.contains_key("get"));
    assert!(t1.commands.contains_key("set"));
    assert!(t1.commands.contains_key("pr.view"));

    // Non-core commands should not appear
    assert!(!t1.commands.contains_key("list"));
    assert!(!t1.commands.contains_key("login"));
    assert!(!t1.commands.contains_key("pr.create"));
    assert!(!t1.commands.contains_key("pr.merge"));
    assert!(!t1.commands.contains_key("issue.list"));
}

#[test]
fn tier1_args_summary() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    // get: <KEY> [--id STRING]
    let get_cmd = &t1.commands["get"];
    assert_eq!(get_cmd.args, "<KEY> [--id STRING]");

    // set: <KEY> [VALUE]
    let set_cmd = &t1.commands["set"];
    assert_eq!(set_cmd.args, "<KEY> [VALUE]");

    // pr.view: <NUMBER> [--json]
    let pr_view = &t1.commands["pr.view"];
    assert_eq!(pr_view.args, "<NUMBER> [--json]");
}

#[test]
fn tier1_returns_summary() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    // get returns {key, type, value}
    assert_eq!(t1.commands["get"].returns, "{key, type, value}");

    // set returns {key, value}
    assert_eq!(t1.commands["set"].returns, "{key, value}");

    // pr.view returns {number, title, state}
    assert_eq!(t1.commands["pr.view"].returns, "{number, title, state}");
}

#[test]
fn tier1_readonly_and_mutation_flags() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    // get is readonly
    assert!(t1.commands["get"].readonly);
    assert!(!t1.commands["get"].mutating);
    assert!(!t1.commands["get"].destructive);

    // set is mutating
    assert!(!t1.commands["set"].readonly);
    assert!(t1.commands["set"].mutating);
    assert!(!t1.commands["set"].destructive);
}

#[test]
fn tier1_pathway_compression() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    // check-state: list -> get <KEY>
    assert_eq!(t1.pathways["check-state"], "list -> get <KEY>");

    // create-pr: pr.create <TITLE> -> pr.view <NUMBER>
    assert_eq!(
        t1.pathways["create-pr"],
        "pr.create <TITLE> -> pr.view <NUMBER>"
    );
}

#[test]
fn tier1_errors() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);

    assert!(t1.errors.contains(&"not_found".to_string()));
    assert!(t1.errors.contains(&"auth_required".to_string()));
    assert!(t1.errors.contains(&"connection (retryable)".to_string()));
}

// =========================================================================
// Tier 2 tests
// =========================================================================

#[test]
fn tier2_command_detail() {
    let manifest = rich_manifest();
    let t2 = project_tier2_command(&manifest, "get").expect("get must exist");

    assert_eq!(t2.command, "get");
    assert_eq!(t2.description, "Get a value by key");
    assert!(!t2.mutating);
    assert!(!t2.destructive);
    assert!(!t2.interactive);
    assert!(t2.non_interactive_alternative.is_none());

    // Args
    assert_eq!(t2.args.len(), 1);
    assert_eq!(t2.args[0].name, "key");
    assert!(t2.args[0].required);

    // Flags
    assert_eq!(t2.flags.len(), 1);
    assert_eq!(t2.flags[0].name, "--id");

    // Output
    let output = t2.output.as_ref().expect("output must exist");
    assert_eq!(output.output_type, "object");
    assert_eq!(output.fields.len(), 3);

    // Errors resolved from global taxonomy
    assert_eq!(t2.errors.len(), 2);
    assert_eq!(t2.errors[0].kind, "not_found");
    assert!(!t2.errors[0].retryable);
    assert_eq!(t2.errors[1].kind, "connection");
    assert!(t2.errors[1].retryable);
}

#[test]
fn tier2_prefers_agent_description() {
    let manifest = rich_manifest();
    let t2 = project_tier2_command(&manifest, "set").expect("set must exist");

    // set has agent_description = "Store a value under a key"
    assert_eq!(t2.description, "Store a value under a key");
}

#[test]
fn tier2_interactive_command() {
    let manifest = rich_manifest();
    let t2 = project_tier2_command(&manifest, "login").expect("login must exist");

    assert!(t2.interactive);
    assert_eq!(
        t2.non_interactive_alternative.as_deref(),
        Some("tt login --token <TOKEN>")
    );
}

#[test]
fn tier2_error_resolution() {
    let manifest = rich_manifest();
    let t2 = project_tier2_command(&manifest, "pr.merge").expect("pr.merge must exist");

    // Should have not_found and auth_required errors
    let auth_err = t2
        .errors
        .iter()
        .find(|e| e.kind == "auth_required")
        .expect("auth_required error must be resolved");
    assert_eq!(
        auth_err.resolution.as_deref(),
        Some("Run 'tt auth login' first")
    );
}

#[test]
fn tier2_nonexistent_command() {
    let manifest = rich_manifest();
    assert!(project_tier2_command(&manifest, "nonexistent").is_none());
}

// =========================================================================
// Token budget tests
// =========================================================================

#[test]
fn tier0_token_budget() {
    let manifest = rich_manifest();
    let t0 = project_tier0(&manifest);
    let json = serde_json::to_string(&t0).expect("serialize tier 0");
    let tokens = estimate_tokens(&json);

    // Budget: ~150-300 tokens. Allow some headroom for the test manifest
    // which has more commands than a typical single tool.
    assert!(
        tokens <= 400,
        "Tier 0 should be compact; got {tokens} tokens ({} bytes): {json}",
        json.len()
    );
}

#[test]
fn tier1_token_budget() {
    let manifest = rich_manifest();
    let t1 = project_tier1(&manifest);
    let json = serde_json::to_string(&t1).expect("serialize tier 1");
    let tokens = estimate_tokens(&json);

    // Budget: ~600 tokens. Our test manifest has 3 core commands + pathways + errors.
    assert!(
        tokens <= 800,
        "Tier 1 should be compact; got {tokens} tokens ({} bytes): {json}",
        json.len()
    );
}

// =========================================================================
// Empty manifest tests (S2)
// =========================================================================

/// Helper: build a completely empty manifest (no commands, no pathways,
/// no agent config).
fn empty_manifest() -> Manifest {
    Manifest {
        schema: None,
        name: "empty".to_string(),
        bin: "empty".to_string(),
        version: semver::Version::new(0, 0, 0),
        description: "An empty manifest".to_string(),
        base_command: vec![],
        agent: None,
        context: None,
        tiers: None,
        pathways: vec![],
        errors: vec![],
        commands: BTreeMap::new(),
    }
}

#[test]
fn tier0_empty_manifest() {
    let manifest = empty_manifest();
    let t0 = project_tier0(&manifest);

    assert_eq!(t0.tool, "empty");
    assert!(t0.groups.is_empty());
    assert!(t0.commands.is_empty());
    assert!(t0.interactive_commands.is_empty());
    assert!(t0.agent_output.is_none());
    assert!(t0.pathways.is_empty());
}

#[test]
fn tier1_empty_manifest() {
    let manifest = empty_manifest();
    let t1 = project_tier1(&manifest);

    assert!(
        t1.commands.is_empty(),
        "no tiers defined should produce empty commands map"
    );
    assert!(t1.pathways.is_empty());
    assert!(t1.errors.is_empty());
}

#[test]
fn tier2_empty_manifest() {
    let manifest = empty_manifest();
    assert!(
        project_tier2_command(&manifest, "anything").is_none(),
        "empty manifest should return None for any command"
    );
}

// =========================================================================
// Tier 2 unresolved error fallback test (C2)
// =========================================================================

#[test]
fn tier2_unresolved_error_produces_fallback() {
    let mut manifest = rich_manifest();

    // Add a command that references an error kind not in the global taxonomy
    manifest.commands.insert(
        "quirky".to_string(),
        Command {
            description: "A quirky command".to_string(),
            agent_description: None,
            mutating: false,
            destructive: false,
            interactive: false,
            non_interactive_alternative: None,
            args: vec![],
            flags: vec![],
            prerequisites: vec![],
            output: None,
            errors: vec!["not_found".to_string(), "totally_unknown".to_string()],
        },
    );

    let t2 = project_tier2_command(&manifest, "quirky").expect("quirky must exist");

    // Should have both errors -- the known one resolved, the unknown one as fallback
    assert_eq!(t2.errors.len(), 2);

    let resolved = &t2.errors[0];
    assert_eq!(resolved.kind, "not_found");
    assert!(!resolved.description.is_empty());

    let fallback = &t2.errors[1];
    assert_eq!(fallback.kind, "totally_unknown");
    assert!(!fallback.retryable);
    assert!(fallback.description.is_empty());
    assert!(fallback.resolution.is_none());
}

// =========================================================================
// Multi-level dot notation test (W4)
// =========================================================================

#[test]
fn tier0_multi_level_dot_notation() {
    let mut manifest = empty_manifest();
    manifest.commands.insert(
        "auth.login.web".to_string(),
        Command {
            description: "Web-based login".to_string(),
            agent_description: None,
            mutating: false,
            destructive: false,
            interactive: true,
            non_interactive_alternative: None,
            args: vec![],
            flags: vec![],
            prerequisites: vec![],
            output: None,
            errors: vec![],
        },
    );

    let t0 = project_tier0(&manifest);

    // Should group under "auth" with leaf "login.web"
    let auth_group = t0.groups.get("auth").expect("auth group must exist");
    assert_eq!(auth_group.commands, vec!["login.web".to_string()]);

    // Should NOT appear in top-level commands
    assert!(!t0.commands.contains(&"auth.login.web".to_string()));
}
