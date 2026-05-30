//! Integration tests for the `tsukai-manifest` CLI binary.
//!
//! These drive the real compiled binary (via `CARGO_BIN_EXE_tsukai-manifest`)
//! so they exercise argument parsing, exit codes, and end-to-end output
//! exactly as a user or agent would experience them.

use std::path::PathBuf;
use std::process::Command;

/// Absolute path to the compiled CLI binary under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_tsukai-manifest")
}

/// Absolute path to the canonical `examples/mx-kv.json` manifest at the
/// workspace root.
fn mx_kv_example() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/tsukai-manifest-cli -> workspace root
    path.pop();
    path.pop();
    path.push("examples");
    path.push("mx-kv.json");
    path
}

/// Write `contents` to a uniquely named temp file and return its path. The
/// process id plus a caller-supplied tag keeps parallel test runs isolated.
fn temp_file(tag: &str, contents: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "tsukai-cli-test-{}-{}.json",
        std::process::id(),
        tag
    ));
    std::fs::write(&path, contents).expect("write temp file");
    path
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("spawn tsukai-manifest binary")
}

// ── validate ────────────────────────────────────────────────────────────

/// Issue #8 / #7 acceptance: `validate examples/mx-kv.json` succeeds.
#[test]
fn validate_mx_kv_example_succeeds() {
    let example = mx_kv_example();
    let out = run(&["validate", example.to_str().unwrap()]);

    assert!(
        out.status.success(),
        "expected exit 0 for the canonical example; stdout: {} stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("is valid"), "stdout was: {stdout}");
}

#[test]
fn validate_mx_kv_example_json_reports_no_errors() {
    let example = mx_kv_example();
    let out = run(&["--json", "validate", example.to_str().unwrap()]);
    assert!(out.status.success());

    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("validate --json emits JSON");
    assert_eq!(value["ok"], serde_json::Value::Bool(true));
    assert_eq!(value["errors"].as_array().unwrap().len(), 0);
    assert_eq!(
        value["warnings"].as_array().unwrap().len(),
        0,
        "the canonical example is expected to have zero warnings"
    );
}

#[test]
fn validate_semantically_invalid_manifest_exits_nonzero() {
    // Structurally well-formed, but a tier references a nonexistent command
    // (validation rule 1) and a command references an undefined error kind
    // (rule 3).
    let bad = temp_file(
        "semantic-bad",
        r#"{
            "name": "bad",
            "bin": "bad",
            "version": "0.1.0",
            "description": "Semantically invalid manifest",
            "tiers": { "core": ["ghost"] },
            "commands": {
                "real": { "description": "A real command", "errors": ["undefined_kind"] }
            }
        }"#,
    );

    let out = run(&["validate", bad.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "semantic validation errors must produce a non-zero exit"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ghost"), "stdout was: {stdout}");
    assert!(stdout.contains("undefined_kind"), "stdout was: {stdout}");

    let _ = std::fs::remove_file(&bad);
}

#[test]
fn validate_invalid_manifest_json_reports_errors() {
    let bad = temp_file(
        "semantic-bad-json",
        r#"{
            "name": "bad",
            "bin": "bad",
            "version": "0.1.0",
            "description": "Semantically invalid manifest",
            "tiers": { "core": ["ghost"] }
        }"#,
    );

    let out = run(&["--json", "validate", bad.to_str().unwrap()]);
    assert!(!out.status.success());

    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("validate --json emits JSON");
    assert_eq!(value["ok"], serde_json::Value::Bool(false));
    assert!(!value["errors"].as_array().unwrap().is_empty());

    let _ = std::fs::remove_file(&bad);
}

#[test]
fn validate_missing_file_is_clean_error_not_panic() {
    let out = run(&["validate", "/definitely/not/a/real/path.json"]);
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("failed to read"), "stderr was: {stderr}");
    // A panic would print "panicked at"; a clean error must not.
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
}

#[test]
fn validate_malformed_json_is_clean_error_not_panic() {
    let bad = temp_file("malformed", "{ this is not valid json");

    let out = run(&["validate", bad.to_str().unwrap()]);
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a valid manifest"),
        "stderr was: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");

    let _ = std::fs::remove_file(&bad);
}

// ── project ───────────────────────────────────────────────────────────────

#[test]
fn project_tier0_emits_discovery_json() {
    let example = mx_kv_example();
    let out = run(&["project", example.to_str().unwrap(), "--tier", "0"]);
    assert!(out.status.success());

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("tier 0 emits JSON");
    assert_eq!(value["tool"], "mx-kv");
    // Flat command surface: get/set/keys are top-level commands.
    let commands = value["commands"].as_array().unwrap();
    assert!(commands.iter().any(|c| c == "get"));
    assert!(commands.iter().any(|c| c == "set"));
    assert!(commands.iter().any(|c| c == "keys"));
}

#[test]
fn project_tier1_emits_core_command_summaries() {
    let example = mx_kv_example();
    let out = run(&["project", example.to_str().unwrap(), "--tier", "1"]);
    assert!(out.status.success());

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("tier 1 emits JSON");
    let commands = value["commands"].as_object().unwrap();
    assert!(commands.contains_key("get"));
    assert!(commands.contains_key("set"));
    assert!(commands.contains_key("keys"));
    // get is read-only.
    assert_eq!(commands["get"]["readonly"], serde_json::Value::Bool(true));
}

#[test]
fn project_tier2_requires_command_name() {
    let example = mx_kv_example();
    let out = run(&["project", example.to_str().unwrap(), "--tier", "2"]);
    assert!(
        !out.status.success(),
        "tier 2 without --command must fail cleanly"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("requires --command"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"));
}

#[test]
fn project_tier2_emits_full_command_detail() {
    let example = mx_kv_example();
    let out = run(&[
        "project",
        example.to_str().unwrap(),
        "--tier",
        "2",
        "--command",
        "reset",
    ]);
    assert!(out.status.success());

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("tier 2 emits JSON");
    assert_eq!(value["command"], "reset");
    assert_eq!(value["destructive"], serde_json::Value::Bool(true));
    assert_eq!(value["mutating"], serde_json::Value::Bool(true));
}

#[test]
fn project_tier2_unknown_command_is_clean_error() {
    let example = mx_kv_example();
    let out = run(&[
        "project",
        example.to_str().unwrap(),
        "--tier",
        "2",
        "--command",
        "does-not-exist",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does not exist"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"));
}

#[test]
fn project_invalid_tier_value_is_rejected_by_clap() {
    let example = mx_kv_example();
    let out = run(&["project", example.to_str().unwrap(), "--tier", "9"]);
    assert!(
        !out.status.success(),
        "an out-of-range tier must be rejected by argument parsing"
    );
}

// ── schema ────────────────────────────────────────────────────────────────

#[test]
fn schema_emits_valid_json_schema() {
    let out = run(&["schema"]);
    assert!(out.status.success());

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("schema emits JSON");
    assert_eq!(
        value["$id"],
        "https://tsukai.yaoyorozu.industries/manifest/v1.json"
    );
    assert!(value["properties"].is_object());
}

#[test]
fn schema_matches_on_disk_schema() {
    // Guards the staleness contract from the CLI side: `schema -o` must
    // reproduce the committed schema byte-for-byte.
    let mut on_disk = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    on_disk.pop();
    on_disk.pop();
    on_disk.push("schemas");
    on_disk.push("manifest.v1.json");

    let expected = std::fs::read_to_string(&on_disk).expect("read committed schema");

    let out_path = temp_file("schema-out", "");
    let out = run(&["schema", "-o", out_path.to_str().unwrap()]);
    assert!(out.status.success());

    let written = std::fs::read_to_string(&out_path).expect("read written schema");
    assert_eq!(
        written, expected,
        "CLI-generated schema must match the committed schema"
    );

    let _ = std::fs::remove_file(&out_path);
}
