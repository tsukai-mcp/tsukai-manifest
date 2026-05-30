//! Integration tests anchored on the canonical `examples/mx-kv.json` manifest.
//!
//! This is the proving-ground fixture from issue #7. It must parse cleanly,
//! validate with zero errors, round-trip through serialization, and project
//! through every tier without panicking.

use std::path::PathBuf;

use tsukai_manifest::{Manifest, project_tier0, project_tier1, project_tier2_command, validate};

fn example_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/tsukai-manifest -> workspace root
    path.pop();
    path.pop();
    path.push("examples");
    path.push("mx-kv.json");
    path
}

fn load() -> Manifest {
    let raw = std::fs::read_to_string(example_path()).expect("read examples/mx-kv.json");
    serde_json::from_str(&raw).expect("examples/mx-kv.json must deserialize into Manifest")
}

#[test]
fn mx_kv_example_deserializes() {
    let manifest = load();
    assert_eq!(manifest.name, "mx-kv");
    assert_eq!(manifest.bin, "mx");
    assert_eq!(manifest.base_command, vec!["mx", "kv"]);
    assert!(manifest.commands.contains_key("get"));
    assert!(manifest.commands.contains_key("set"));
    assert!(manifest.commands.contains_key("keys"));
}

#[test]
fn mx_kv_example_validates_with_zero_errors() {
    let manifest = load();
    let result = validate(&manifest);
    assert!(
        result.is_valid(),
        "examples/mx-kv.json must validate cleanly, got: {result}"
    );
    assert!(
        !result.has_warnings(),
        "examples/mx-kv.json is expected to emit no warnings, got: {result}"
    );
}

#[test]
fn mx_kv_example_round_trips() {
    let manifest = load();
    let serialized = serde_json::to_string(&manifest).expect("serialize");
    let reparsed: Manifest = serde_json::from_str(&serialized).expect("reparse");
    assert_eq!(manifest, reparsed);
}

#[test]
fn mx_kv_example_projects_through_all_tiers() {
    let manifest = load();

    let t0 = project_tier0(&manifest);
    assert_eq!(t0.tool, "mx-kv");

    let t1 = project_tier1(&manifest);
    // Core tier is get/set/keys.
    assert!(t1.commands.contains_key("get"));
    assert!(t1.commands.contains_key("set"));
    assert!(t1.commands.contains_key("keys"));

    // Every command must project to Tier 2 without panicking.
    for name in manifest.commands.keys() {
        assert!(
            project_tier2_command(&manifest, name).is_some(),
            "tier 2 projection failed for {name}"
        );
    }
}

#[test]
fn mx_kv_example_exercises_mutation_markers() {
    let manifest = load();

    let get = &manifest.commands["get"];
    assert!(!get.mutating && !get.destructive, "get is read-only");

    let set = &manifest.commands["set"];
    assert!(set.mutating && !set.destructive, "set is mutating");

    let reset = &manifest.commands["reset"];
    assert!(
        reset.mutating && reset.destructive,
        "reset is destructive (and therefore mutating)"
    );
}

#[test]
fn mx_kv_example_carries_worked_examples() {
    // Issue #18 field: at least one command must demonstrate `examples`.
    let manifest = load();
    let total: usize = manifest.commands.values().map(|c| c.examples.len()).sum();
    assert!(
        total > 0,
        "the proving-ground manifest should carry worked examples"
    );
}
