//! Round-trip and schema-conformance tests (issue #14).
//!
//! These tests validate manifest instances against the **in-memory generated**
//! JSON Schema — `generate_manifest_schema()` — rather than the on-disk
//! `schemas/manifest.v1.json`. This keeps them orthogonal to the
//! `schema_staleness` test: staleness guards the committed file, these guard
//! that real manifests actually conform to what the types generate and that
//! serialization is lossless.
//!
//! The generated schema carries a canonical `$id`
//! (`https://tsukai.yaoyorozu.industries/manifest/v1.json`) which does not
//! resolve over the network. We therefore build the validator directly from the
//! schema value with `jsonschema::validator_for`, which compiles the document
//! in place without attempting remote `$ref`/`$id` resolution.

use std::collections::BTreeMap;
use std::path::PathBuf;

use jsonschema::Validator;
use semver::Version;
use serde_json::{Value, json};
use tsukai_manifest::{
    Arg, Command, Flag, Manifest, PathwayArg, PathwayStep, generate_manifest_schema, project_tier1,
};

fn example_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/tsukai-manifest -> workspace root
    path.pop();
    path.pop();
    path.push("examples");
    path.push("mx-kv.json");
    path
}

fn load_example() -> Manifest {
    let raw = std::fs::read_to_string(example_path()).expect("read examples/mx-kv.json");
    serde_json::from_str(&raw).expect("examples/mx-kv.json must deserialize into Manifest")
}

fn build_validator() -> Validator {
    let schema = generate_manifest_schema();
    jsonschema::validator_for(&schema).expect("generated schema must compile into a validator")
}

/// Assert that `instance` is valid, printing every schema error on failure.
fn assert_valid(validator: &Validator, instance: &Value, context: &str) {
    if !validator.is_valid(instance) {
        let errors: Vec<String> = validator
            .iter_errors(instance)
            .map(|e| format!("  at {}: {e}", e.instance_path))
            .collect();
        panic!(
            "{context}: instance failed schema validation:\n{}",
            errors.join("\n")
        );
    }
}

#[test]
fn example_manifest_validates_against_generated_schema() {
    let validator = build_validator();
    let manifest = load_example();

    let instance = serde_json::to_value(&manifest).expect("serialize manifest to value");
    assert_valid(&validator, &instance, "examples/mx-kv.json");

    // Round-trip: value -> Manifest -> value preserves the model exactly.
    let reparsed: Manifest = serde_json::from_value(instance).expect("from_value round-trip");
    assert_eq!(
        manifest, reparsed,
        "example manifest must round-trip losslessly"
    );
}

#[test]
fn minimal_manifest_validates_against_generated_schema() {
    let validator = build_validator();

    let manifest = Manifest {
        schema: None,
        name: "minimal".to_string(),
        bin: "min".to_string(),
        version: Version::new(0, 1, 0),
        description: "A minimal valid manifest".to_string(),
        base_command: vec![],
        agent: None,
        context: None,
        tiers: None,
        pathways: vec![],
        errors: vec![],
        commands: BTreeMap::new(),
    };

    let instance = serde_json::to_value(&manifest).expect("serialize manifest to value");
    assert_valid(&validator, &instance, "minimal manifest");

    let reparsed: Manifest = serde_json::from_value(instance).expect("from_value round-trip");
    assert_eq!(
        manifest, reparsed,
        "minimal manifest must round-trip losslessly"
    );
}

#[test]
fn invalid_manifest_fails_schema_validation() {
    let validator = build_validator();
    let base = serde_json::to_value(load_example()).expect("serialize example");

    // Corruption 1: `version` as a number instead of a semver string.
    let mut bad_version = base.clone();
    bad_version["version"] = json!(1);
    assert!(
        !validator.is_valid(&bad_version),
        "version as a number must fail schema validation"
    );

    // Corruption 2: a PathwayArg with an unknown discriminant `kind`.
    let mut bad_kind = base.clone();
    bad_kind["pathways"][0]["steps"][1]["args"] =
        json!([{ "kind": "bogus", "name": "key", "value": "<KEY>" }]);
    assert!(
        !validator.is_valid(&bad_kind),
        "a PathwayArg with kind:\"bogus\" must fail schema validation"
    );

    // Corruption 3: drop a required field (`name`) from the top level.
    let mut missing_name = base.clone();
    missing_name
        .as_object_mut()
        .expect("manifest is an object")
        .remove("name");
    assert!(
        !validator.is_valid(&missing_name),
        "a manifest missing the required `name` field must fail schema validation"
    );
}

/// Issue #25 regression, asserted directly: the `track-history` pathway must
/// project to exactly the ordered string with the flag rendered as
/// `--count 5`.
#[test]
fn track_history_tier1_renders_ordered_flag() {
    let manifest = load_example();
    let t1 = project_tier1(&manifest);
    assert_eq!(
        t1.pathways["track-history"],
        "push <KEY> <VALUE> -> last <KEY> --count 5"
    );
}

/// A hand-built manifest exercising both PathwayArg variants validates and
/// round-trips. Guards the tagged-enum serde shape end to end.
#[test]
fn pathway_arg_variants_round_trip_through_schema() {
    let validator = build_validator();

    let manifest = Manifest {
        schema: None,
        name: "t".to_string(),
        bin: "t".to_string(),
        version: Version::new(0, 1, 0),
        description: "t".to_string(),
        base_command: vec![],
        agent: None,
        context: None,
        tiers: None,
        pathways: vec![tsukai_manifest::Pathway {
            name: "p".to_string(),
            description: "p".to_string(),
            prerequisites: vec![],
            steps: vec![PathwayStep {
                command: "get".to_string(),
                args: vec![
                    PathwayArg::Positional {
                        name: "key".to_string(),
                        value: "<KEY>".to_string(),
                    },
                    PathwayArg::Flag {
                        name: "--id".to_string(),
                        value: Some("5".to_string()),
                    },
                    PathwayArg::Flag {
                        name: "--json".to_string(),
                        value: None,
                    },
                ],
                note: None,
            }],
        }],
        errors: vec![],
        commands: BTreeMap::from([(
            "get".to_string(),
            Command {
                description: "Get".to_string(),
                agent_description: None,
                mutating: false,
                destructive: false,
                interactive: false,
                non_interactive_alternative: None,
                args: vec![Arg {
                    name: "key".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: "Key".to_string(),
                    default: None,
                    enum_values: None,
                    constraints: None,
                }],
                flags: vec![
                    Flag {
                        name: "--id".to_string(),
                        flag_type: "string".to_string(),
                        required: false,
                        description: "ID".to_string(),
                        default: None,
                    },
                    Flag {
                        name: "--json".to_string(),
                        flag_type: "boolean".to_string(),
                        required: false,
                        description: "JSON".to_string(),
                        default: None,
                    },
                ],
                prerequisites: vec![],
                output: None,
                examples: vec![],
                errors: vec![],
            },
        )]),
    };

    let instance = serde_json::to_value(&manifest).expect("serialize");
    assert_valid(&validator, &instance, "pathway-arg manifest");

    let reparsed: Manifest = serde_json::from_value(instance).expect("from_value round-trip");
    assert_eq!(manifest, reparsed);
}
