//! Semantic validation for manifests.
//!
//! This module validates invariants that JSON Schema structural checks cannot
//! express: referential integrity between commands and tiers, error kind
//! references, pathway step validity, duplicate detection, and other
//! cross-cutting constraints.
//!
//! # Usage
//!
//! ```
//! use tsukai_manifest::validation::validate;
//! # use tsukai_manifest::Manifest;
//! # let manifest: Manifest = serde_json::from_str(r#"{"name":"t","bin":"t","version":"0.1.0","description":"t"}"#).unwrap();
//! let result = validate(&manifest);
//! if result.is_valid() {
//!     // No errors — manifest is semantically sound (may still have warnings)
//! }
//! ```

use std::collections::HashSet;

use crate::Manifest;
use crate::manifest::PathwayArg;

/// A semantic validation error — a hard failure that must be fixed.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    /// Dotted path to the offending field (e.g. `"tiers.core[2]"`,
    /// `"commands.get.errors[0]"`).
    pub path: String,

    /// Human-readable description of the problem.
    pub message: String,
}

/// A semantic validation warning — advisory, not a blocker.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationWarning {
    /// Dotted path to the relevant field.
    pub path: String,

    /// Human-readable description of the concern.
    pub message: String,
}

/// Aggregated result of semantic validation.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Hard failures that must be resolved.
    pub errors: Vec<ValidationError>,

    /// Advisory notices that don't block validity.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Returns `true` when there are no errors. Warnings alone don't
    /// invalidate a manifest.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns `true` when there is at least one warning.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error at {}: {}", self.path, self.message)
    }
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "warning at {}: {}", self.path, self.message)
    }
}

impl std::fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.errors {
            writeln!(f, "{e}")?;
        }
        for w in &self.warnings {
            writeln!(f, "{w}")?;
        }
        Ok(())
    }
}

/// Run all semantic validation rules against a manifest.
///
/// Returns a [`ValidationResult`] containing any errors and warnings. A
/// manifest is considered valid when `result.is_valid()` is `true` (no
/// errors). Warnings are informational and do not block validity.
pub fn validate(manifest: &Manifest) -> ValidationResult {
    let mut result = ValidationResult::default();

    validate_tier_references(manifest, &mut result);
    validate_tier_overlap(manifest, &mut result);
    validate_error_references(manifest, &mut result);
    validate_prerequisite_references(manifest, &mut result);
    validate_interactive_consistency(manifest, &mut result);
    validate_destructive_implies_mutating(manifest, &mut result);
    validate_pathway_step_references(manifest, &mut result);
    validate_pathway_step_arg_references(manifest, &mut result);
    validate_no_duplicate_arg_flag_names(manifest, &mut result);
    validate_self_command_groups(manifest, &mut result);
    validate_pathway_names(manifest, &mut result);
    validate_pathway_args(manifest, &mut result);
    validate_pathway_has_steps(manifest, &mut result);
    // Rule 9 (valid semver) is enforced by the type system: `Manifest.version`
    // is `semver::Version`, which only deserializes valid semver strings.
    // See `test_version_enforced_by_type_system` below for confirmation.

    result
}

/// Rule 1: Every command name in tiers must exist in `commands`.
fn validate_tier_references(manifest: &Manifest, result: &mut ValidationResult) {
    let Some(tiers) = &manifest.tiers else {
        return;
    };

    let tier_lists: &[(&str, &[String])] = &[
        ("core", &tiers.core),
        ("common", &tiers.common),
        ("extended", &tiers.extended),
    ];

    for (tier_name, commands) in tier_lists {
        for (i, cmd_name) in commands.iter().enumerate() {
            if !manifest.commands.contains_key(cmd_name) {
                result.errors.push(ValidationError {
                    path: format!("tiers.{tier_name}[{i}]"),
                    message: format!(
                        "tier references command \"{cmd_name}\" which does not exist in commands"
                    ),
                });
            }
        }
    }
}

/// Rule 2: A command must not appear in multiple tiers.
fn validate_tier_overlap(manifest: &Manifest, result: &mut ValidationResult) {
    let Some(tiers) = &manifest.tiers else {
        return;
    };

    let tier_lists: &[(&str, &[String])] = &[
        ("core", &tiers.core),
        ("common", &tiers.common),
        ("extended", &tiers.extended),
    ];

    let mut seen: HashSet<&str> = HashSet::new();
    let mut seen_in_tier: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();

    for (tier_name, commands) in tier_lists {
        for (i, cmd_name) in commands.iter().enumerate() {
            if !seen.insert(cmd_name.as_str()) {
                let other_tier = seen_in_tier[cmd_name.as_str()];
                let message = if other_tier == *tier_name {
                    format!(
                        "command \"{cmd_name}\" is listed more than once in the \"{tier_name}\" tier"
                    )
                } else {
                    format!(
                        "command \"{cmd_name}\" appears in both \"{other_tier}\" and \"{tier_name}\" tiers"
                    )
                };
                result.errors.push(ValidationError {
                    path: format!("tiers.{tier_name}[{i}]"),
                    message,
                });
            } else {
                seen_in_tier.insert(cmd_name.as_str(), tier_name);
            }
        }
    }
}

/// Rule 3: Every error kind in a command's `errors` must exist in the
/// top-level `errors` array.
fn validate_error_references(manifest: &Manifest, result: &mut ValidationResult) {
    let known_kinds: HashSet<&str> = manifest.errors.iter().map(|e| e.kind.as_str()).collect();

    for (cmd_name, cmd) in &manifest.commands {
        for (i, error_kind) in cmd.errors.iter().enumerate() {
            if !known_kinds.contains(error_kind.as_str()) {
                result.errors.push(ValidationError {
                    path: format!("commands.{cmd_name}.errors[{i}]"),
                    message: format!(
                        "references error kind \"{error_kind}\" which is not defined in the top-level errors array"
                    ),
                });
            }
        }
    }
}

/// Rule 4: Command and pathway prerequisites reference existing commands.
fn validate_prerequisite_references(manifest: &Manifest, result: &mut ValidationResult) {
    for (cmd_name, cmd) in &manifest.commands {
        for (i, prereq) in cmd.prerequisites.iter().enumerate() {
            if prereq == cmd_name {
                result.errors.push(ValidationError {
                    path: format!("commands.{cmd_name}.prerequisites[{i}]"),
                    message: format!("command \"{cmd_name}\" lists itself as a prerequisite"),
                });
            } else if !manifest.commands.contains_key(prereq) {
                result.errors.push(ValidationError {
                    path: format!("commands.{cmd_name}.prerequisites[{i}]"),
                    message: format!("prerequisite \"{prereq}\" does not exist in commands"),
                });
            }
        }
    }

    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        for (i, prereq) in pathway.prerequisites.iter().enumerate() {
            if !manifest.commands.contains_key(prereq) {
                result.errors.push(ValidationError {
                    path: format!("pathways[{pi}].prerequisites[{i}]"),
                    message: format!(
                        "pathway \"{}\" prerequisite \"{prereq}\" does not exist in commands",
                        pathway.name
                    ),
                });
            }
        }
    }
}

/// Rule 5: If `interactive: true` and no `non_interactive_alternative`,
/// emit a warning (not error).
fn validate_interactive_consistency(manifest: &Manifest, result: &mut ValidationResult) {
    for (cmd_name, cmd) in &manifest.commands {
        if cmd.interactive && cmd.non_interactive_alternative.is_none() {
            result.warnings.push(ValidationWarning {
                path: format!("commands.{cmd_name}"),
                message: format!(
                    "command \"{cmd_name}\" is interactive but has no non_interactive_alternative \
                     — agents cannot invoke it"
                ),
            });
        }
    }
}

/// Rule 6: If `destructive: true`, `mutating` must also be true.
fn validate_destructive_implies_mutating(manifest: &Manifest, result: &mut ValidationResult) {
    for (cmd_name, cmd) in &manifest.commands {
        if cmd.destructive && !cmd.mutating {
            result.errors.push(ValidationError {
                path: format!("commands.{cmd_name}"),
                message: format!(
                    "command \"{cmd_name}\" is destructive but not marked as mutating"
                ),
            });
        }
    }
}

/// Rule 7: Each pathway step's `command` must exist in `commands`.
fn validate_pathway_step_references(manifest: &Manifest, result: &mut ValidationResult) {
    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        for (si, step) in pathway.steps.iter().enumerate() {
            if !manifest.commands.contains_key(&step.command) {
                result.errors.push(ValidationError {
                    path: format!("pathways[{pi}].steps[{si}].command"),
                    message: format!(
                        "pathway \"{}\" step references command \"{}\" which does not exist",
                        pathway.name, step.command
                    ),
                });
            }
        }
    }
}

/// Rule 7b: Each pathway step argument must reference an argument or flag that
/// the step's command actually defines. A `Positional` arg name must match one
/// of the command's `args[].name`; a `Flag` arg name must match one of the
/// command's `flags[].name` (flag names carry their `--` prefix in both the
/// pathway and the command definition). A pathway referencing a non-existent
/// arg/flag is a real defect — it would render an invocation the tool cannot
/// accept. Steps whose `command` does not exist are skipped here; Rule 7 already
/// reports those.
fn validate_pathway_step_arg_references(manifest: &Manifest, result: &mut ValidationResult) {
    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        for (si, step) in pathway.steps.iter().enumerate() {
            let Some(cmd) = manifest.commands.get(&step.command) else {
                continue;
            };

            for (ai, arg) in step.args.iter().enumerate() {
                match arg {
                    PathwayArg::Positional { name, .. } => {
                        if !cmd.args.iter().any(|a| a.name == *name) {
                            result.errors.push(ValidationError {
                                path: format!("pathways[{pi}].steps[{si}].args[{ai}]"),
                                message: format!(
                                    "pathway \"{}\" step \"{}\" passes positional argument \"{name}\" \
                                     which is not declared in that command's args",
                                    pathway.name, step.command
                                ),
                            });
                        }
                    }
                    PathwayArg::Flag { name, .. } => {
                        if !cmd.flags.iter().any(|f| f.name == *name) {
                            result.errors.push(ValidationError {
                                path: format!("pathways[{pi}].steps[{si}].args[{ai}]"),
                                message: format!(
                                    "pathway \"{}\" step \"{}\" passes flag \"{name}\" \
                                     which is not declared in that command's flags",
                                    pathway.name, step.command
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Rule 8: No duplicate arg or flag names within a single command.
fn validate_no_duplicate_arg_flag_names(manifest: &Manifest, result: &mut ValidationResult) {
    for (cmd_name, cmd) in &manifest.commands {
        let mut seen: HashSet<&str> = HashSet::new();

        for (i, arg) in cmd.args.iter().enumerate() {
            if !seen.insert(arg.name.as_str()) {
                result.errors.push(ValidationError {
                    path: format!("commands.{cmd_name}.args[{i}]"),
                    message: format!("duplicate argument name \"{}\"", arg.name),
                });
            }
        }

        for (i, flag) in cmd.flags.iter().enumerate() {
            if !seen.insert(flag.name.as_str()) {
                result.errors.push(ValidationError {
                    path: format!("commands.{cmd_name}.flags[{i}]"),
                    message: format!("duplicate flag name \"{}\"", flag.name),
                });
            }
        }
    }
}

/// Rule 10: A command name that is both a bare command and a group prefix
/// (e.g. `remote` exists alongside `remote.add`) is valid — it becomes the
/// group's namespace-default command (cf. `git remote`). Emit a warning, not
/// an error, in case the overlap is unintended.
fn validate_self_command_groups(manifest: &Manifest, result: &mut ValidationResult) {
    let group_prefixes: HashSet<&str> = manifest
        .commands
        .keys()
        .filter_map(|key| key.split_once('.').map(|(prefix, _)| prefix))
        .collect();

    for key in manifest.commands.keys() {
        if !key.contains('.') && group_prefixes.contains(key.as_str()) {
            result.warnings.push(ValidationWarning {
                path: format!("commands.{key}"),
                message: format!(
                    "command \"{key}\" is both a bare command and a group prefix; this is valid \
                     (it becomes the group's self_command, cf. `git remote`) but flagged in case \
                     the overlap is unintended"
                ),
            });
        }
    }
}

/// Rules 11 and 13: pathway name integrity.
///
/// Rule 11 (error): pathway names must be unique across `pathways`.
/// Rule 13 (warning): a pathway name should match
/// `^[A-Za-z0-9][A-Za-z0-9_-]*$` — bridges derive tool names from pathway
/// names and may skip pathways whose names fall outside this charset.
fn validate_pathway_names(manifest: &Manifest, result: &mut ValidationResult) {
    let mut seen: HashSet<&str> = HashSet::new();

    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        if !seen.insert(pathway.name.as_str()) {
            result.errors.push(ValidationError {
                path: format!("pathways[{pi}].name"),
                message: format!("duplicate pathway name \"{}\"", pathway.name),
            });
        }

        if !pathway_name_is_well_formed(&pathway.name) {
            result.warnings.push(ValidationWarning {
                path: format!("pathways[{pi}].name"),
                message: format!(
                    "pathway name \"{}\" does not match ^[A-Za-z0-9][A-Za-z0-9_-]*$ — bridges \
                     may skip it when deriving tool names",
                    pathway.name
                ),
            });
        }
    }
}

fn pathway_name_is_well_formed(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Rules 12a–12f: pathway arg declarations and placeholder integrity.
///
/// A placeholder is a maximal substring `<ident>` where `ident` matches
/// `[A-Za-z0-9_][A-Za-z0-9_-]*`; it resolves to a declared pathway arg by
/// case-insensitive name match. Substitution by the bridge must be total, so:
///
/// - 12a (error): arg names must be unique case-insensitively within a pathway.
/// - 12b (error): an optional arg (`required: false`) must carry a `default`.
///   A `required: true` arg may carry a `default`; it passes silently — the
///   default is unused, the caller must supply the value.
/// - 12c (error): when the pathway declares args, every placeholder in step
///   arg values must resolve to a declared arg.
/// - 12d (warning): a declared arg whose placeholder appears in no step is a
///   dead parameter. Args that failed 12f are skipped here — one diagnostic
///   per defect, the most specific one.
/// - 12e (warning): placeholder-shaped tokens in step values of a pathway
///   that declares no args pass through verbatim (legacy/illustrative).
/// - 12f (error): arg names must match the placeholder ident grammar
///   `[A-Za-z0-9_][A-Za-z0-9_-]*` (uppercase deliberately allowed — `Key` is
///   referenceable via case-insensitive resolution). 12b ensures every
///   declared arg has a value; 12c ensures every placeholder has an arg; 12f
///   ensures every arg can have a placeholder. Without it the bridge
///   advertises an inputSchema demanding a value that structurally never
///   reaches any step.
fn validate_pathway_args(manifest: &Manifest, result: &mut ValidationResult) {
    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        let mut seen: HashSet<String> = HashSet::new();
        let mut unreferenceable: HashSet<usize> = HashSet::new();
        for (ai, arg) in pathway.args.iter().enumerate() {
            if !is_placeholder_ident(&arg.name) {
                unreferenceable.insert(ai);
                result.errors.push(ValidationError {
                    path: format!("pathways[{pi}].args[{ai}].name"),
                    message: format!(
                        "pathway \"{}\" arg \"{}\" cannot be referenced by any placeholder; \
                         arg names must match [A-Za-z0-9_][A-Za-z0-9_-]*",
                        pathway.name, arg.name
                    ),
                });
            }

            if !seen.insert(arg.name.to_ascii_lowercase()) {
                result.errors.push(ValidationError {
                    path: format!("pathways[{pi}].args[{ai}]"),
                    message: format!(
                        "pathway \"{}\" declares duplicate argument name \"{}\" (argument names \
                         are matched case-insensitively)",
                        pathway.name, arg.name
                    ),
                });
            }

            if !arg.required && arg.default.is_none() {
                result.errors.push(ValidationError {
                    path: format!("pathways[{pi}].args[{ai}]"),
                    message: format!(
                        "pathway \"{}\" argument \"{}\" is optional (required: false) but has no \
                         default — placeholder substitution must be total",
                        pathway.name, arg.name
                    ),
                });
            }
        }

        let declared: HashSet<String> = pathway
            .args
            .iter()
            .map(|a| a.name.to_ascii_lowercase())
            .collect();
        let mut used: HashSet<String> = HashSet::new();

        for (si, step) in pathway.steps.iter().enumerate() {
            for (ai, step_arg) in step.args.iter().enumerate() {
                let value = match step_arg {
                    PathwayArg::Positional { value, .. } => value.as_str(),
                    PathwayArg::Flag {
                        value: Some(value), ..
                    } => value.as_str(),
                    PathwayArg::Flag { value: None, .. } => continue,
                };

                for ident in placeholders(value) {
                    if pathway.args.is_empty() {
                        result.warnings.push(ValidationWarning {
                            path: format!("pathways[{pi}].steps[{si}].args[{ai}]"),
                            message: format!(
                                "pathway \"{}\" declares no args but a step value contains the \
                                 placeholder-shaped token \"<{ident}>\" — it will pass through \
                                 verbatim",
                                pathway.name
                            ),
                        });
                    } else if declared.contains(&ident.to_ascii_lowercase()) {
                        used.insert(ident.to_ascii_lowercase());
                    } else {
                        result.errors.push(ValidationError {
                            path: format!("pathways[{pi}].steps[{si}].args[{ai}]"),
                            message: format!(
                                "pathway \"{}\" step value placeholder \"<{ident}>\" does not \
                                 resolve to any declared pathway arg",
                                pathway.name
                            ),
                        });
                    }
                }
            }
        }

        for (ai, arg) in pathway.args.iter().enumerate() {
            // An arg that failed 12f already drew the more specific
            // "unreferenceable" error; reporting it dead too is noise.
            if unreferenceable.contains(&ai) {
                continue;
            }
            if !used.contains(&arg.name.to_ascii_lowercase()) {
                result.warnings.push(ValidationWarning {
                    path: format!("pathways[{pi}].args[{ai}]"),
                    message: format!(
                        "pathway \"{}\" argument \"{}\" placeholder appears in no step value — \
                         dead parameter",
                        pathway.name, arg.name
                    ),
                });
            }
        }
    }
}

/// Returns `true` when `name` matches the placeholder ident grammar
/// `[A-Za-z0-9_][A-Za-z0-9_-]*` — byte-identical to what [`placeholders`]
/// extracts, so a matching arg name is always referenceable.
fn is_placeholder_ident(name: &str) -> bool {
    let bytes = name.as_bytes();
    match bytes.first() {
        Some(&b) if b.is_ascii_alphanumeric() || b == b'_' => {}
        _ => return false,
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Rule 14: A pathway with zero steps provides no guidance.
fn validate_pathway_has_steps(manifest: &Manifest, result: &mut ValidationResult) {
    for (pi, pathway) in manifest.pathways.iter().enumerate() {
        if pathway.steps.is_empty() {
            result.warnings.push(ValidationWarning {
                path: format!("pathways[{pi}].steps"),
                message: format!("pathway \"{}\" has zero steps", pathway.name),
            });
        }
    }
}

/// Yields each placeholder identifier in `value`, without the angle brackets.
///
/// A placeholder is a maximal substring `<ident>` where `ident` matches
/// `[A-Za-z0-9_][A-Za-z0-9_-]*`. Anything else — `<>`, `<-x>`, an unclosed
/// `<ident` — is not a placeholder and is skipped.
fn placeholders(value: &str) -> impl Iterator<Item = &str> {
    let bytes = value.as_bytes();
    let mut idx = 0;

    std::iter::from_fn(move || {
        while idx < bytes.len() {
            if bytes[idx] != b'<' {
                idx += 1;
                continue;
            }

            let start = idx + 1;
            let mut end = start;
            if end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric()
                        || bytes[end] == b'_'
                        || bytes[end] == b'-')
                {
                    end += 1;
                }
                if end < bytes.len() && bytes[end] == b'>' {
                    idx = end + 1;
                    return Some(&value[start..end]);
                }
            }

            idx = start;
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use semver::Version;

    use super::*;
    use crate::{Arg, Command, ErrorDef, Flag, Manifest, Pathway, PathwayArg, PathwayStep, Tiers};

    /// Builds a minimal valid manifest that passes all validation rules.
    fn valid_manifest() -> Manifest {
        Manifest {
            schema: None,
            name: "test-tool".to_string(),
            bin: "test".to_string(),
            version: Version::new(0, 1, 0),
            description: "A test tool".to_string(),
            base_command: vec![],
            agent: None,
            context: None,
            tiers: Some(Tiers {
                core: vec!["get".to_string()],
                common: vec!["set".to_string()],
                extended: vec!["delete".to_string()],
            }),
            pathways: vec![Pathway {
                name: "read-write".to_string(),
                description: "Read then write".to_string(),
                args: vec![Arg {
                    name: "key".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: "Key to read".to_string(),
                    default: None,
                    enum_values: None,
                    constraints: None,
                }],
                prerequisites: vec!["get".to_string()],
                steps: vec![
                    PathwayStep {
                        command: "get".to_string(),
                        args: vec![PathwayArg::Positional {
                            name: "key".to_string(),
                            value: "<KEY>".to_string(),
                        }],
                        note: None,
                    },
                    PathwayStep {
                        command: "set".to_string(),
                        args: vec![],
                        note: None,
                    },
                ],
            }],
            errors: vec![ErrorDef {
                kind: "not_found".to_string(),
                retryable: false,
                description: "Not found".to_string(),
                resolution: None,
            }],
            commands: BTreeMap::from([
                (
                    "get".to_string(),
                    Command {
                        description: "Get a value".to_string(),
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
                        flags: vec![Flag {
                            name: "--json".to_string(),
                            flag_type: "boolean".to_string(),
                            required: false,
                            description: "JSON output".to_string(),
                            default: None,
                        }],
                        prerequisites: vec![],
                        output: None,
                        examples: vec![],
                        errors: vec!["not_found".to_string()],
                    },
                ),
                (
                    "set".to_string(),
                    Command {
                        description: "Set a value".to_string(),
                        agent_description: None,
                        mutating: true,
                        destructive: false,
                        interactive: false,
                        non_interactive_alternative: None,
                        args: vec![],
                        flags: vec![],
                        prerequisites: vec!["get".to_string()],
                        output: None,
                        examples: vec![],
                        errors: vec![],
                    },
                ),
                (
                    "delete".to_string(),
                    Command {
                        description: "Delete a key".to_string(),
                        agent_description: None,
                        mutating: true,
                        destructive: true,
                        interactive: false,
                        non_interactive_alternative: None,
                        args: vec![],
                        flags: vec![],
                        prerequisites: vec![],
                        output: None,
                        examples: vec![],
                        errors: vec![],
                    },
                ),
            ]),
        }
    }

    // ── Fully valid manifest passes ──────────────────────────────────

    #[test]
    fn valid_manifest_passes_all_rules() {
        let result = validate(&valid_manifest());
        assert!(result.is_valid(), "expected no errors but got: {result}");
        assert!(
            !result.has_warnings(),
            "expected no warnings but got: {result}"
        );
    }

    #[test]
    fn empty_manifest_is_valid() {
        let manifest = Manifest {
            schema: None,
            name: "empty".to_string(),
            bin: "empty".to_string(),
            version: Version::new(0, 1, 0),
            description: "Empty".to_string(),
            base_command: vec![],
            agent: None,
            context: None,
            tiers: None,
            pathways: vec![],
            errors: vec![],
            commands: BTreeMap::new(),
        };
        let result = validate(&manifest);
        assert!(result.is_valid());
        assert!(!result.has_warnings());
    }

    // ── Rule 1: Tier references ──────────────────────────────────────

    #[test]
    fn tier_reference_to_existing_command_is_valid() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn tier_reference_to_missing_command_is_error() {
        let mut m = valid_manifest();
        m.tiers
            .as_mut()
            .unwrap()
            .core
            .push("nonexistent".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.starts_with("tiers.core"))
            .unwrap();
        assert!(err.path.contains("core[1]"));
        assert!(err.message.contains("nonexistent"));
    }

    // ── Rule 2: Tier overlap ─────────────────────────────────────────

    #[test]
    fn no_tier_overlap_is_valid() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn command_in_two_tiers_is_error() {
        let mut m = valid_manifest();
        // "get" is already in core, add it to common too
        m.tiers.as_mut().unwrap().common.push("get".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.message.contains("appears in both"))
            .unwrap();
        assert!(err.message.contains("get"));
        assert!(err.message.contains("core"));
        assert!(err.message.contains("common"));
    }

    // ── Rule 3: Error references ─────────────────────────────────────

    #[test]
    fn valid_error_references_pass() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn invalid_error_reference_is_error() {
        let mut m = valid_manifest();
        m.commands
            .get_mut("get")
            .unwrap()
            .errors
            .push("bogus_error".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.get.errors"))
            .unwrap();
        assert!(err.path.contains("errors[1]"));
        assert!(err.message.contains("bogus_error"));
    }

    // ── Rule 4: Prerequisite references ──────────────────────────────

    #[test]
    fn valid_command_prerequisites_pass() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn command_prerequisite_to_missing_command_is_error() {
        let mut m = valid_manifest();
        m.commands
            .get_mut("set")
            .unwrap()
            .prerequisites
            .push("missing_cmd".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.set.prerequisites"))
            .unwrap();
        assert!(err.message.contains("missing_cmd"));
    }

    #[test]
    fn pathway_prerequisite_to_missing_command_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].prerequisites.push("ghost".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("pathways[0].prerequisites"))
            .unwrap();
        assert!(err.message.contains("ghost"));
    }

    // ── Rule 5: Interactive consistency ──────────────────────────────

    #[test]
    fn interactive_with_alternative_no_warning() {
        let mut m = valid_manifest();
        m.commands.get_mut("get").unwrap().interactive = true;
        m.commands
            .get_mut("get")
            .unwrap()
            .non_interactive_alternative = Some("get --stdin".to_string());
        let result = validate(&m);
        assert!(!result.has_warnings(), "unexpected warning: {result}");
    }

    #[test]
    fn interactive_without_alternative_is_warning() {
        let mut m = valid_manifest();
        m.commands.get_mut("get").unwrap().interactive = true;
        m.commands
            .get_mut("get")
            .unwrap()
            .non_interactive_alternative = None;
        let result = validate(&m);
        assert!(result.is_valid(), "should still be valid (warnings only)");
        assert!(result.has_warnings());
        let warn = result
            .warnings
            .iter()
            .find(|w| w.path.contains("commands.get"))
            .unwrap();
        assert!(warn.message.contains("interactive"));
        assert!(warn.message.contains("non_interactive_alternative"));
    }

    // ── Rule 6: Destructive implies mutating ─────────────────────────

    #[test]
    fn destructive_and_mutating_is_valid() {
        let m = valid_manifest();
        // "delete" is already destructive: true, mutating: true
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn destructive_without_mutating_is_error() {
        let mut m = valid_manifest();
        m.commands.get_mut("delete").unwrap().mutating = false;
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.delete"))
            .unwrap();
        assert!(err.message.contains("destructive"));
        assert!(err.message.contains("mutating"));
    }

    // ── Rule 7: Pathway step references ──────────────────────────────

    #[test]
    fn pathway_steps_referencing_valid_commands_pass() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn pathway_step_referencing_missing_command_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].steps.push(PathwayStep {
            command: "vanished".to_string(),
            args: vec![],
            note: None,
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("pathways[0].steps[2].command"))
            .unwrap();
        assert!(err.message.contains("vanished"));
    }

    // ── Rule 7b: Pathway step arg references ─────────────────────────

    #[test]
    fn pathway_step_with_valid_positional_and_flag_passes() {
        let mut m = valid_manifest();
        // `get` declares arg `key` and flag `--json`.
        m.pathways[0].steps[0].args = vec![
            PathwayArg::Positional {
                name: "key".to_string(),
                value: "<KEY>".to_string(),
            },
            PathwayArg::Flag {
                name: "--json".to_string(),
                value: None,
            },
        ];
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
    }

    #[test]
    fn pathway_step_positional_referencing_unknown_arg_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "not_a_real_arg".to_string(),
            value: "x".to_string(),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("pathways[0].steps[0].args[0]"))
            .unwrap();
        assert!(err.message.contains("not_a_real_arg"));
        assert!(err.message.contains("positional"));
    }

    #[test]
    fn pathway_step_flag_referencing_unknown_flag_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].steps[0].args = vec![PathwayArg::Flag {
            name: "--bogus".to_string(),
            value: Some("1".to_string()),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("pathways[0].steps[0].args[0]"))
            .unwrap();
        assert!(err.message.contains("--bogus"));
        assert!(err.message.contains("flag"));
    }

    #[test]
    fn pathway_step_positional_name_must_not_match_a_flag() {
        let mut m = valid_manifest();
        // `--json` is a flag on `get`, not a positional arg. Using it as a
        // positional must fail.
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "--json".to_string(),
            value: "x".to_string(),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("pathways[0].steps[0].args[0]"))
            .unwrap();
        assert!(err.message.contains("positional"));
    }

    // ── Rule 8: No duplicate arg/flag names ──────────────────────────

    #[test]
    fn unique_arg_flag_names_pass() {
        let m = valid_manifest();
        let result = validate(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn duplicate_arg_name_is_error() {
        let mut m = valid_manifest();
        let cmd = m.commands.get_mut("get").unwrap();
        cmd.args.push(Arg {
            name: "key".to_string(),
            arg_type: "string".to_string(),
            required: false,
            description: "Duplicate".to_string(),
            default: None,
            enum_values: None,
            constraints: None,
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.get.args[1]"))
            .unwrap();
        assert!(err.message.contains("duplicate"));
        assert!(err.message.contains("key"));
    }

    #[test]
    fn duplicate_flag_name_is_error() {
        let mut m = valid_manifest();
        let cmd = m.commands.get_mut("get").unwrap();
        cmd.flags.push(Flag {
            name: "--json".to_string(),
            flag_type: "boolean".to_string(),
            required: false,
            description: "Duplicate".to_string(),
            default: None,
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.get.flags[1]"))
            .unwrap();
        assert!(err.message.contains("duplicate"));
        assert!(err.message.contains("--json"));
    }

    #[test]
    fn arg_and_flag_with_same_name_is_error() {
        let mut m = valid_manifest();
        let cmd = m.commands.get_mut("get").unwrap();
        // arg named "key" already exists; add a flag also named "key"
        cmd.flags.push(Flag {
            name: "key".to_string(),
            flag_type: "string".to_string(),
            required: false,
            description: "Collides with arg".to_string(),
            default: None,
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.message.contains("duplicate") && e.message.contains("key"))
            .unwrap();
        assert!(err.path.contains("flags"));
    }

    // ── Rule 9: Semver enforced by type system ───────────────────────

    #[test]
    fn test_version_enforced_by_type_system() {
        // Valid semver deserializes fine
        let json = r#"{"name":"t","bin":"t","version":"1.2.3","description":"t"}"#;
        assert!(serde_json::from_str::<Manifest>(json).is_ok());

        // Invalid semver is rejected at deserialization
        let bad_json = r#"{"name":"t","bin":"t","version":"not-semver","description":"t"}"#;
        assert!(serde_json::from_str::<Manifest>(bad_json).is_err());

        let also_bad = r#"{"name":"t","bin":"t","version":"1.2","description":"t"}"#;
        assert!(serde_json::from_str::<Manifest>(also_bad).is_err());
    }

    // ── Display / helper method tests ────────────────────────────────

    #[test]
    fn validation_result_display_includes_all_issues() {
        let result = ValidationResult {
            errors: vec![ValidationError {
                path: "tiers.core[0]".to_string(),
                message: "bad ref".to_string(),
            }],
            warnings: vec![ValidationWarning {
                path: "commands.x".to_string(),
                message: "no alternative".to_string(),
            }],
        };
        let output = format!("{result}");
        assert!(output.contains("error at tiers.core[0]: bad ref"));
        assert!(output.contains("warning at commands.x: no alternative"));
    }

    #[test]
    fn is_valid_and_has_warnings_orthogonal() {
        // No errors, no warnings
        let empty = ValidationResult::default();
        assert!(empty.is_valid());
        assert!(!empty.has_warnings());

        // Warnings only
        let warn_only = ValidationResult {
            errors: vec![],
            warnings: vec![ValidationWarning {
                path: "x".to_string(),
                message: "y".to_string(),
            }],
        };
        assert!(warn_only.is_valid());
        assert!(warn_only.has_warnings());

        // Errors only
        let err_only = ValidationResult {
            errors: vec![ValidationError {
                path: "x".to_string(),
                message: "y".to_string(),
            }],
            warnings: vec![],
        };
        assert!(!err_only.is_valid());
        assert!(!err_only.has_warnings());
    }

    // ── Rule 2 addendum: within-tier duplicates ──────────────────────

    #[test]
    fn command_duplicated_within_same_tier_is_error() {
        let mut m = valid_manifest();
        m.tiers.as_mut().unwrap().core.push("get".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("tiers.core"))
            .unwrap();
        assert!(
            err.message.contains("listed more than once"),
            "within-tier duplicate should say 'listed more than once', got: {}",
            err.message
        );
    }

    // ── Rule 10: self_command group overlap is a warning ────────────

    #[test]
    fn bare_command_matching_group_prefix_is_warning_not_error() {
        let mut m = valid_manifest();
        // `delete` already exists as a bare command. Add `delete.all` so
        // `delete` is also a group prefix.
        m.commands.insert(
            "delete.all".to_string(),
            Command {
                description: "Delete everything".to_string(),
                agent_description: None,
                mutating: true,
                destructive: true,
                interactive: false,
                non_interactive_alternative: None,
                args: vec![],
                flags: vec![],
                prerequisites: vec![],
                output: None,
                examples: vec![],
                errors: vec![],
            },
        );
        // Keep tiers referentially valid by registering the new command.
        m.tiers
            .as_mut()
            .unwrap()
            .extended
            .push("delete.all".to_string());

        let result = validate(&m);
        assert!(
            result.is_valid(),
            "self_command overlap must not be an error: {result}"
        );
        assert!(result.has_warnings());
        let warn = result
            .warnings
            .iter()
            .find(|w| w.path == "commands.delete")
            .expect("expected a warning for the bare/group overlap");
        assert!(warn.message.contains("self_command"));
        assert!(warn.message.contains("git remote"));
    }

    #[test]
    fn group_prefix_without_bare_command_no_warning() {
        let mut m = valid_manifest();
        // `issue.list` makes `issue` a group prefix, but there is no bare
        // `issue` command, so no overlap warning should fire.
        m.commands.insert(
            "issue.list".to_string(),
            Command {
                description: "List issues".to_string(),
                agent_description: None,
                mutating: false,
                destructive: false,
                interactive: false,
                non_interactive_alternative: None,
                args: vec![],
                flags: vec![],
                prerequisites: vec![],
                output: None,
                examples: vec![],
                errors: vec![],
            },
        );
        m.tiers
            .as_mut()
            .unwrap()
            .common
            .push("issue.list".to_string());

        let result = validate(&m);
        assert!(
            !result
                .warnings
                .iter()
                .any(|w| w.message.contains("self_command")),
            "no self_command warning expected: {result}"
        );
    }

    // ── Rule 4 addendum: self-referencing prerequisites ─────────────

    #[test]
    fn self_referencing_prerequisite_is_error() {
        let mut m = valid_manifest();
        m.commands
            .get_mut("get")
            .unwrap()
            .prerequisites
            .push("get".to_string());
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path.contains("commands.get.prerequisites"))
            .unwrap();
        assert!(err.message.contains("itself"));
    }

    // ── Rule 8 addendum: case sensitivity ───────────────────────────

    #[test]
    fn arg_names_are_case_sensitive() {
        let mut m = valid_manifest();
        let cmd = m.commands.get_mut("get").unwrap();
        // "key" already exists; "Key" is a different name
        cmd.args.push(Arg {
            name: "Key".to_string(),
            arg_type: "string".to_string(),
            required: false,
            description: "Different case".to_string(),
            default: None,
            enum_values: None,
            constraints: None,
        });
        let result = validate(&m);
        assert!(
            result.is_valid(),
            "\"key\" and \"Key\" should be treated as distinct names, got: {result}"
        );
    }

    // ── Multiple violations at once ──────────────────────────────────

    #[test]
    fn multiple_violations_all_reported() {
        let mut m = valid_manifest();

        // Break several rules at once
        m.tiers.as_mut().unwrap().core.push("phantom".to_string()); // rule 1
        m.commands
            .get_mut("get")
            .unwrap()
            .errors
            .push("phantom_err".to_string()); // rule 3
        m.commands.get_mut("delete").unwrap().mutating = false; // rule 6

        let result = validate(&m);
        assert!(!result.is_valid());

        // All three should be reported
        assert!(
            result.errors.len() >= 3,
            "expected at least 3 errors, got {}: {result}",
            result.errors.len()
        );
    }

    // ── Rule 11: Duplicate pathway names ─────────────────────────────

    #[test]
    fn distinct_pathway_names_pass() {
        let mut m = valid_manifest();
        m.pathways.push(Pathway {
            name: "write-only".to_string(),
            description: "Just write".to_string(),
            args: vec![],
            prerequisites: vec![],
            steps: vec![PathwayStep {
                command: "set".to_string(),
                args: vec![],
                note: None,
            }],
        });
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    #[test]
    fn duplicate_pathway_name_is_error() {
        let mut m = valid_manifest();
        m.pathways.push(Pathway {
            name: "read-write".to_string(),
            description: "Same name again".to_string(),
            args: vec![],
            prerequisites: vec![],
            steps: vec![PathwayStep {
                command: "set".to_string(),
                args: vec![],
                note: None,
            }],
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[1].name")
            .unwrap();
        assert!(err.message.contains("duplicate pathway name"));
        assert!(err.message.contains("read-write"));
    }

    // ── Rule 12a: Pathway arg names unique case-insensitively ───────

    #[test]
    fn pathway_args_with_distinct_names_pass() {
        let mut m = valid_manifest();
        m.pathways[0].args.push(Arg {
            name: "count".to_string(),
            arg_type: "integer".to_string(),
            required: true,
            description: "How many".to_string(),
            default: None,
            enum_values: None,
            constraints: None,
        });
        // Use both placeholders so neither arg is a dead parameter.
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "<KEY>-<COUNT>".to_string(),
        }];
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    #[test]
    fn pathway_arg_names_duplicate_case_insensitively_is_error() {
        let mut m = valid_manifest();
        // "key" is already declared; "KEY" differs only by case.
        m.pathways[0].args.push(Arg {
            name: "KEY".to_string(),
            arg_type: "string".to_string(),
            required: true,
            description: "Case-colliding duplicate".to_string(),
            default: None,
            enum_values: None,
            constraints: None,
        });
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].args[1]")
            .unwrap();
        assert!(err.message.contains("duplicate argument name"));
        assert!(err.message.contains("case-insensitively"));
        assert!(err.message.contains("KEY"));
    }

    // ── Rule 12b: Optional pathway arg requires a default ───────────

    #[test]
    fn optional_pathway_arg_with_default_passes() {
        let mut m = valid_manifest();
        m.pathways[0].args[0].required = false;
        m.pathways[0].args[0].default = Some(serde_json::json!("focus"));
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    #[test]
    fn optional_pathway_arg_without_default_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].args[0].required = false;
        m.pathways[0].args[0].default = None;
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].args[0]")
            .unwrap();
        assert!(err.message.contains("optional"));
        assert!(err.message.contains("no default"));
    }

    #[test]
    fn required_arg_with_default_passes_silently() {
        // Pinned posture: required: true plus a default is accepted — the
        // default is simply unused, the caller must supply the value.
        let mut m = valid_manifest();
        m.pathways[0].args[0].required = true;
        m.pathways[0].args[0].default = Some(serde_json::json!("focus"));
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    // ── Rule 12f: Arg names must match the placeholder ident grammar ─

    #[test]
    fn uppercase_arg_name_is_referenceable_and_valid() {
        let mut m = valid_manifest();
        // The fixture step value "<KEY>" resolves to the uppercase name
        // case-insensitively; uppercase is deliberately allowed.
        m.pathways[0].args[0].name = "KEY".to_string();
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    #[test]
    fn non_ascii_arg_name_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].args[0].name = "clé".to_string();
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "literal".to_string(),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].args[0].name")
            .unwrap();
        assert!(err.message.contains("clé"));
        assert!(err.message.contains("cannot be referenced"));
        assert!(err.message.contains("[A-Za-z0-9_][A-Za-z0-9_-]*"));
    }

    #[test]
    fn arg_name_with_space_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].args[0].name = "my key".to_string();
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "literal".to_string(),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].args[0].name")
            .unwrap();
        assert!(err.message.contains("my key"));
        assert!(err.message.contains("[A-Za-z0-9_][A-Za-z0-9_-]*"));
    }

    #[test]
    fn unreferenceable_arg_suppresses_12d_but_valid_unused_arg_still_warns() {
        let mut m = valid_manifest();
        // "clé" fails 12f (the more specific diagnostic — no 12d on top);
        // "orphan" is charset-valid and unused, so 12d still fires for it.
        m.pathways[0].args = vec![
            Arg {
                name: "clé".to_string(),
                arg_type: "string".to_string(),
                required: true,
                description: "Unreferenceable".to_string(),
                default: None,
                enum_values: None,
                constraints: None,
            },
            Arg {
                name: "orphan".to_string(),
                arg_type: "string".to_string(),
                required: true,
                description: "Valid but unused".to_string(),
                default: None,
                enum_values: None,
                constraints: None,
            },
        ];
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "literal".to_string(),
        }];
        let result = validate(&m);
        assert_eq!(result.errors.len(), 1, "only the 12f error: {result}");
        assert_eq!(result.errors[0].path, "pathways[0].args[0].name");
        assert_eq!(result.warnings.len(), 1, "only orphan's 12d: {result}");
        assert_eq!(result.warnings[0].path, "pathways[0].args[1]");
        assert!(result.warnings[0].message.contains("orphan"));
        assert!(
            !result.warnings.iter().any(|w| w.message.contains("clé")),
            "12f-failing arg must not also be reported dead: {result}"
        );
    }

    // ── Rule 12c: Placeholders must resolve to declared args ────────

    #[test]
    fn placeholder_resolves_case_insensitively() {
        let mut m = valid_manifest();
        // The declared arg is "key"; the fixture already uses "<KEY>". Flip
        // the placeholder to lowercase to cover the other direction.
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "<key>".to_string(),
        }];
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert!(!result.has_warnings(), "expected no warnings: {result}");
    }

    #[test]
    fn ghost_placeholder_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].steps[0].args = vec![PathwayArg::Positional {
            name: "key".to_string(),
            value: "<GHOST>".to_string(),
        }];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].steps[0].args[0]")
            .unwrap();
        assert!(err.message.contains("<GHOST>"));
        assert!(err.message.contains("does not resolve"));
    }

    #[test]
    fn ghost_placeholder_in_flag_value_is_error() {
        let mut m = valid_manifest();
        m.pathways[0].steps[0].args = vec![
            PathwayArg::Positional {
                name: "key".to_string(),
                value: "<KEY>".to_string(),
            },
            PathwayArg::Flag {
                name: "--json".to_string(),
                value: Some("<GHOST>".to_string()),
            },
        ];
        let result = validate(&m);
        assert!(!result.is_valid());
        let err = result
            .errors
            .iter()
            .find(|e| e.path == "pathways[0].steps[0].args[1]")
            .unwrap();
        assert!(err.message.contains("<GHOST>"));
    }

    // ── Rule 12d: Dead pathway parameter ─────────────────────────────

    #[test]
    fn declared_arg_unused_in_any_step_is_warning() {
        let mut m = valid_manifest();
        m.pathways[0].args.push(Arg {
            name: "orphan".to_string(),
            arg_type: "string".to_string(),
            required: true,
            description: "Never referenced".to_string(),
            default: None,
            enum_values: None,
            constraints: None,
        });
        let result = validate(&m);
        assert!(result.is_valid(), "should still be valid (warnings only)");
        let warn = result
            .warnings
            .iter()
            .find(|w| w.path == "pathways[0].args[1]")
            .unwrap();
        assert!(warn.message.contains("orphan"));
        assert!(warn.message.contains("appears in no step"));
    }

    // ── Rule 12e: Placeholder tokens without declared args ──────────

    #[test]
    fn legacy_manifest_with_placeholder_tokens_and_no_args_warns_only() {
        let mut m = valid_manifest();
        // Legacy shape: step values carry "<KEY>" but the pathway declares
        // no args. Must stay valid with exactly the 12e warning.
        m.pathways[0].args = vec![];
        let result = validate(&m);
        assert!(result.is_valid(), "expected no errors, got: {result}");
        assert_eq!(
            result.warnings.len(),
            1,
            "expected only the rule-12e warning, got: {result}"
        );
        let warn = &result.warnings[0];
        assert_eq!(warn.path, "pathways[0].steps[0].args[0]");
        assert!(warn.message.contains("declares no args"));
        assert!(warn.message.contains("<KEY>"));
    }

    // ── Rule 13: Pathway name charset ────────────────────────────────

    #[test]
    fn well_formed_pathway_name_no_warning() {
        let mut m = valid_manifest();
        m.pathways[0].name = "Check_state-2".to_string();
        let result = validate(&m);
        assert!(!result.has_warnings(), "unexpected warning: {result}");
    }

    #[test]
    fn malformed_pathway_name_is_warning() {
        let mut m = valid_manifest();
        m.pathways[0].name = "bad name!".to_string();
        let result = validate(&m);
        assert!(result.is_valid(), "should still be valid (warnings only)");
        let warn = result
            .warnings
            .iter()
            .find(|w| w.path == "pathways[0].name")
            .unwrap();
        assert!(warn.message.contains("does not match"));
        assert!(warn.message.contains("bad name!"));
    }

    #[test]
    fn pathway_name_starting_with_separator_is_warning() {
        let mut m = valid_manifest();
        m.pathways[0].name = "-leading-dash".to_string();
        let result = validate(&m);
        assert!(result.is_valid());
        assert!(
            result.warnings.iter().any(|w| w.path == "pathways[0].name"),
            "leading '-' must trip the charset warning: {result}"
        );
    }

    // ── Rule 14: Pathway with zero steps ─────────────────────────────

    #[test]
    fn pathway_with_steps_no_warning() {
        let result = validate(&valid_manifest());
        assert!(!result.has_warnings(), "unexpected warning: {result}");
    }

    #[test]
    fn pathway_with_zero_steps_is_warning() {
        let mut m = valid_manifest();
        m.pathways.push(Pathway {
            name: "noop".to_string(),
            description: "Does nothing".to_string(),
            args: vec![],
            prerequisites: vec![],
            steps: vec![],
        });
        let result = validate(&m);
        assert!(result.is_valid(), "should still be valid (warnings only)");
        let warn = result
            .warnings
            .iter()
            .find(|w| w.path == "pathways[1].steps")
            .unwrap();
        assert!(warn.message.contains("zero steps"));
        assert!(warn.message.contains("noop"));
    }

    // ── Placeholder extraction helper ────────────────────────────────

    #[test]
    fn placeholders_extracts_maximal_idents() {
        let found: Vec<&str> =
            placeholders("<KEY> mid <a-b_2> <-bad> <> <unclosed and <<NESTED>>").collect();
        assert_eq!(found, vec!["KEY", "a-b_2", "NESTED"]);

        // Multibyte text around a placeholder does not disturb extraction;
        // multibyte inside the ident candidate disqualifies it.
        assert_eq!(placeholders("日本<KEY>語").collect::<Vec<_>>(), vec!["KEY"]);
        assert_eq!(placeholders("<ké>").count(), 0);
    }

    #[test]
    fn placeholders_empty_and_plain_strings_yield_nothing() {
        assert_eq!(placeholders("").count(), 0);
        assert_eq!(placeholders("no tokens here").count(), 0);
        assert_eq!(placeholders("a < b > c").count(), 0);
    }
}
