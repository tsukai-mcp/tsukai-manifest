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
    validate_no_duplicate_arg_flag_names(manifest, &mut result);
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
                result.errors.push(ValidationError {
                    path: format!("tiers.{tier_name}[{i}]"),
                    message: format!(
                        "command \"{cmd_name}\" appears in both \"{other_tier}\" and \"{tier_name}\" tiers"
                    ),
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
            if !manifest.commands.contains_key(prereq) {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use semver::Version;

    use super::*;
    use crate::{Arg, Command, ErrorDef, Flag, Manifest, Pathway, PathwayStep, Tiers};

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
                prerequisites: vec!["get".to_string()],
                steps: vec![
                    PathwayStep {
                        command: "get".to_string(),
                        args: BTreeMap::new(),
                        note: None,
                    },
                    PathwayStep {
                        command: "set".to_string(),
                        args: BTreeMap::new(),
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
            args: BTreeMap::new(),
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
}
