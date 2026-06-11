//! Hostile-input tests for the pathway-exposure validation layer (issue #8 §1):
//! UTF-8 adversarial placeholder candidates, serde posture pinning, rule
//! interaction matrices, round-trip properties, and boundary conditions for
//! rules 11, 12a–12f, 13, and 14.

use tsukai_manifest::{
    Arg, Manifest, Pathway, PathwayArg, PathwayStep, ValidationResult, validate,
};

/// Minimal manifest with one command `run` (positional arg `input`, boolean
/// flag `--json`) so pathway steps pass rules 7/7b and the pathway rules under
/// test are the only variables.
fn base_manifest() -> Manifest {
    serde_json::from_str(
        r#"{
            "name": "t",
            "bin": "t",
            "version": "0.1.0",
            "description": "t",
            "commands": {
                "run": {
                    "description": "Run",
                    "args": [
                        {"name": "input", "type": "string", "required": true, "description": "in"}
                    ],
                    "flags": [
                        {"name": "--json", "type": "boolean", "required": false, "description": "json"}
                    ]
                }
            }
        }"#,
    )
    .expect("base manifest deserializes")
}

fn req_arg(name: &str) -> Arg {
    Arg {
        name: name.to_string(),
        arg_type: "string".to_string(),
        required: true,
        description: format!("arg {name}"),
        default: None,
        enum_values: None,
        constraints: None,
    }
}

fn pathway(name: &str, args: Vec<Arg>, step_value: &str) -> Pathway {
    Pathway {
        name: name.to_string(),
        description: "d".to_string(),
        args,
        prerequisites: vec![],
        steps: vec![PathwayStep {
            command: "run".to_string(),
            args: vec![PathwayArg::Positional {
                name: "input".to_string(),
                value: step_value.to_string(),
            }],
            note: None,
        }],
    }
}

/// Validate a single pathway whose only step carries `step_value`.
fn validate_value(args: Vec<Arg>, step_value: &str) -> ValidationResult {
    let mut m = base_manifest();
    m.pathways = vec![pathway("p", args, step_value)];
    validate(&m)
}

fn assert_clean(result: &ValidationResult) {
    assert!(result.is_valid(), "expected no errors, got: {result}");
    assert!(
        !result.has_warnings(),
        "expected no warnings, got: {result}"
    );
}

// ── UTF-8 hostile placeholder candidates ────────────────────────────────

#[test]
fn multibyte_in_ident_candidate_is_not_a_placeholder() {
    // `é` is outside the ident charset, so `<ké>` is not a placeholder.
    // With no declared args there must be no 12e warning.
    assert_clean(&validate_value(vec![], "<ké>"));

    // A declared arg named `ké` can never be referenced: rule 12f rejects the
    // name outright (error), `<ké>` is not extracted so no 12c fires, and the
    // 12f-failing arg is skipped by the 12d dead-parameter scan.
    let result = validate_value(vec![req_arg("ké")], "<ké>");
    assert_eq!(result.errors.len(), 1, "expected only 12f: {result}");
    assert_eq!(result.errors[0].path, "pathways[0].args[0].name");
    assert!(result.errors[0].message.contains("cannot be referenced"));
    assert!(
        !result.has_warnings(),
        "12f suppresses the 12d warning for the same arg: {result}"
    );
}

#[test]
fn multibyte_text_around_placeholder_still_resolves() {
    assert_clean(&validate_value(vec![req_arg("key")], "é<KEY>é"));
}

#[test]
fn zero_width_joiner_inside_ident_candidate_breaks_it() {
    let result = validate_value(vec![req_arg("key")], "<KE\u{200D}Y>");
    assert!(result.is_valid(), "expected no errors, got: {result}");
    let warn = result
        .warnings
        .iter()
        .find(|w| w.message.contains("appears in no step"))
        .expect("ZWJ must break the ident, leaving the arg dead");
    assert!(warn.message.contains("key"));
}

#[test]
fn unclosed_placeholder_at_eof_is_ignored() {
    let result = validate_value(vec![req_arg("key")], "prefix <KEY");
    assert!(result.is_valid(), "expected no errors, got: {result}");
    assert_eq!(
        result.warnings.len(),
        1,
        "unclosed token must not resolve; arg is dead: {result}"
    );
    assert!(result.warnings[0].message.contains("appears in no step"));
}

#[test]
fn empty_angle_brackets_are_not_a_placeholder() {
    assert_clean(&validate_value(vec![], "a <> b"));
}

#[test]
fn doubled_angle_brackets_resolve_the_inner_ident() {
    // `<<KEY>>` contains the placeholder `<KEY>`; the outer brackets are
    // literal text.
    assert_clean(&validate_value(vec![req_arg("key")], "<<KEY>>"));
}

#[test]
fn trailing_dashes_are_part_of_the_ident() {
    // `<A-->` is a single placeholder with ident `A--`.
    assert_clean(&validate_value(vec![req_arg("a--")], "<A-->"));

    let result = validate_value(vec![], "<A-->");
    assert_eq!(result.warnings.len(), 1, "expected one 12e: {result}");
    assert!(result.warnings[0].message.contains("<A-->"));
}

#[test]
fn leading_dash_is_not_an_ident() {
    assert_clean(&validate_value(vec![], "<-A>"));
}

#[test]
fn single_digit_and_single_underscore_idents_resolve() {
    assert_clean(&validate_value(
        vec![req_arg("9"), req_arg("_")],
        "<9> and <_>",
    ));
}

#[test]
fn hostile_gauntlet_extracts_exactly_the_well_formed_tokens() {
    // Only KEY, NEST, A--, 9, _ are placeholders; everything else is noise.
    let value = "<ké> é<KEY>é <KE\u{200D}Y> <KEY <> <<NEST>> <A--> <-A> <9> <_>";
    let result = validate_value(vec![], value);
    assert!(result.is_valid(), "expected no errors, got: {result}");
    assert_eq!(
        result.warnings.len(),
        5,
        "expected exactly five 12e warnings, got: {result}"
    );
    for token in ["<KEY>", "<NEST>", "<A-->", "<9>", "<_>"] {
        assert!(
            result.warnings.iter().any(|w| w.message.contains(token)),
            "missing 12e warning for {token}: {result}"
        );
    }
}

#[test]
fn megabyte_value_with_ten_thousand_placeholders_resolves_without_panic() {
    // ~1.03 MB single value, 10k resolving placeholders.
    let value = format!("{}<KEY>", "y".repeat(98)).repeat(10_000);
    assert_clean(&validate_value(vec![req_arg("key")], &value));
}

#[test]
fn megabyte_value_with_ten_thousand_ghosts_reports_every_one() {
    let value = format!("{}<GHOST>", "x".repeat(96)).repeat(10_000);
    let result = validate_value(vec![req_arg("key")], &value);
    assert_eq!(
        result.errors.len(),
        10_000,
        "every ghost placeholder must be reported (no early abort)"
    );
    assert!(result.errors.iter().all(|e| e.message.contains("<GHOST>")));
    // The declared arg is never used: 12d still fires alongside 10k errors.
    assert_eq!(result.warnings.len(), 1, "expected only 12d: {result}");
}

// ── Serde posture ────────────────────────────────────────────────────────

fn manifest_json_with_pathway_body(pathway_body: &str) -> String {
    format!(
        r#"{{
            "name": "t", "bin": "t", "version": "0.1.0", "description": "t",
            "pathways": [{pathway_body}],
            "commands": {{
                "run": {{
                    "description": "Run",
                    "args": [{{"name": "input", "type": "string", "required": true, "description": "in"}}]
                }}
            }}
        }}"#
    )
}

#[test]
fn pathway_args_null_is_a_deserialization_error() {
    // serde(default) applies only when the key is absent; an explicit null is
    // a type error, not an empty vec.
    let json = manifest_json_with_pathway_body(
        r#"{"name": "p", "description": "d", "args": null, "steps": []}"#,
    );
    let err = serde_json::from_str::<Manifest>(&json)
        .expect_err("args: null must be rejected, not coerced to []");
    assert!(
        err.to_string().contains("null"),
        "error should name the null: {err}"
    );
}

#[test]
fn pathway_args_absent_and_empty_array_both_yield_empty_vec() {
    let absent =
        manifest_json_with_pathway_body(r#"{"name": "p", "description": "d", "steps": []}"#);
    let empty = manifest_json_with_pathway_body(
        r#"{"name": "p", "description": "d", "args": [], "steps": []}"#,
    );

    let m_absent: Manifest = serde_json::from_str(&absent).expect("absent args");
    let m_empty: Manifest = serde_json::from_str(&empty).expect("empty args");
    assert!(m_absent.pathways[0].args.is_empty());
    assert_eq!(m_absent.pathways[0], m_empty.pathways[0]);

    // And empty args are omitted on the way back out.
    let out = serde_json::to_string(&m_empty.pathways[0]).expect("serialize");
    assert!(!out.contains("args"), "empty args must be omitted: {out}");
}

#[test]
fn duplicate_args_keys_in_pathway_json_are_rejected_not_last_wins() {
    // If serde silently kept the last `args`, validation would never see what
    // the author wrote first. Pin the reject.
    let json = manifest_json_with_pathway_body(
        r#"{
            "name": "p", "description": "d",
            "args": [{"name": "real", "type": "string", "required": true, "description": "r"}],
            "args": [],
            "steps": []
        }"#,
    );
    let err = serde_json::from_str::<Manifest>(&json)
        .expect_err("duplicate `args` keys must be a hard parse error");
    assert!(
        err.to_string().contains("duplicate field"),
        "expected duplicate-field error, got: {err}"
    );
}

#[test]
fn unknown_fields_on_pathway_arg_are_silently_ignored() {
    // Crate posture: forward compatibility, no deny_unknown_fields. Pinned so
    // a future posture change is a conscious decision.
    let json = manifest_json_with_pathway_body(
        r#"{
            "name": "p", "description": "d",
            "args": [{
                "name": "key", "type": "string", "required": true, "description": "k",
                "hostile_extra": {"nested": [1, 2, 3]}
            }],
            "steps": [{"command": "run", "args": [{"kind": "positional", "name": "input", "value": "<KEY>"}]}]
        }"#,
    );
    let m: Manifest = serde_json::from_str(&json).expect("unknown fields must be ignored");
    assert_eq!(m.pathways[0].args[0].name, "key");
    assert_clean(&validate(&m));

    // The unknown field is dropped, not round-tripped.
    let out = serde_json::to_string(&m).expect("serialize");
    assert!(!out.contains("hostile_extra"));
}

#[test]
fn enormous_arg_name_validates_without_panic() {
    let name = "A".repeat(100_000);
    let value = format!("<{name}>");
    assert_clean(&validate_value(vec![req_arg(&name)], &value));
}

#[test]
fn arg_named_with_angle_brackets_can_never_be_referenced() {
    // Declared name `<KEY>` fails the 12f name grammar (error). The extractor
    // yields `KEY` from `<<KEY>>`, which resolves to no declared arg → 12c.
    // The 12f-failing arg draws no additional 12d dead-parameter warning.
    let result = validate_value(vec![req_arg("<KEY>")], "<<KEY>>");
    assert_eq!(result.errors.len(), 2, "expected 12f + 12c: {result}");
    let twelve_f = result
        .errors
        .iter()
        .find(|e| e.path == "pathways[0].args[0].name")
        .expect("12f error for the malformed arg name");
    assert!(twelve_f.message.contains("cannot be referenced"));
    let twelve_c = result
        .errors
        .iter()
        .find(|e| e.message.contains("does not resolve"))
        .expect("12c error for the unresolvable placeholder");
    assert!(twelve_c.message.contains("\"<KEY>\""));
    assert!(
        !result.has_warnings(),
        "12f suppresses 12d for the same arg: {result}"
    );
}

#[test]
fn non_ascii_case_variants_are_distinct_arg_names() {
    // 12a folds with to_ascii_lowercase only: `É` and `é` stay distinct —
    // no duplicate error. Both fail the 12f name grammar instead (each can
    // never be referenced by a placeholder), and 12f suppresses their 12d.
    let result = validate_value(vec![req_arg("É"), req_arg("é")], "plain");
    assert!(
        !result
            .errors
            .iter()
            .any(|e| e.message.contains("duplicate argument name")),
        "ASCII-only folding must not merge É/é into a 12a duplicate: {result}"
    );
    assert_eq!(result.errors.len(), 2, "one 12f error per arg: {result}");
    assert!(
        result
            .errors
            .iter()
            .all(|e| e.message.contains("cannot be referenced")),
        "both errors are 12f: {result}"
    );
    assert!(
        !result.has_warnings(),
        "12f-failing args draw no 12d warnings: {result}"
    );
}

// ── Rule interaction matrix ──────────────────────────────────────────────

#[test]
fn kitchen_sink_pathway_emits_every_diagnostic_without_early_abort() {
    let mut m = base_manifest();

    // p0: charset-violating name (13), case-colliding duplicate arg (12a),
    // optional arg with no default (12b), ghost placeholder (12c), dead
    // parameter (12d via `opt`).
    let mut p0 = pathway(
        "bad name!",
        vec![req_arg("key"), req_arg("KEY")],
        "<KEY> <GHOST>",
    );
    p0.args.push(Arg {
        name: "opt".to_string(),
        arg_type: "string".to_string(),
        required: false,
        description: "optional, no default".to_string(),
        default: None,
        enum_values: None,
        constraints: None,
    });

    // p1: duplicate name (11) + zero steps (14) + charset (13) again.
    let p1 = Pathway {
        name: "bad name!".to_string(),
        description: "dup".to_string(),
        args: vec![],
        prerequisites: vec![],
        steps: vec![],
    };

    m.pathways = vec![p0, p1];
    let result = validate(&m);

    let expect_error = |needle: &str| {
        assert!(
            result.errors.iter().any(|e| e.message.contains(needle)),
            "missing error containing {needle:?}: {result}"
        );
    };
    let expect_warning = |needle: &str| {
        assert!(
            result.warnings.iter().any(|w| w.message.contains(needle)),
            "missing warning containing {needle:?}: {result}"
        );
    };

    expect_error("duplicate pathway name"); // 11
    expect_error("duplicate argument name"); // 12a
    expect_error("no default"); // 12b
    expect_error("<GHOST>"); // 12c
    expect_warning("appears in no step"); // 12d (opt)
    expect_warning("does not match"); // 13
    expect_warning("zero steps"); // 14

    assert_eq!(result.errors.len(), 4, "11 + 12a + 12b + 12c: {result}");
    // 13 fires for both pathways sharing the malformed name.
    assert_eq!(
        result.warnings.len(),
        4,
        "12d + 13 (x2) + 14 expected: {result}"
    );
}

#[test]
fn pathway_name_duplicated_three_times_yields_two_errors() {
    let mut m = base_manifest();
    m.pathways = vec![
        pathway("x", vec![], "a"),
        pathway("x", vec![], "b"),
        pathway("x", vec![], "c"),
    ];
    let result = validate(&m);
    let dup_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.message.contains("duplicate pathway name"))
        .collect();
    assert_eq!(
        dup_errors.len(),
        2,
        "second and third occurrences each report once: {result}"
    );
    assert_eq!(dup_errors[0].path, "pathways[1].name");
    assert_eq!(dup_errors[1].path, "pathways[2].name");
}

#[test]
fn pathway_named_pathway_is_allowed_and_clean() {
    // The bridge namespaces tools as {tool}.pathway.{name}; manifest-side a
    // pathway literally named "pathway" is rule-13-clean and fully valid.
    let mut m = base_manifest();
    m.pathways = vec![pathway("pathway", vec![req_arg("key")], "<KEY>")];
    assert_clean(&validate(&m));
}

// ── Boundary conditions ──────────────────────────────────────────────────

#[test]
fn zero_args_and_zero_placeholders_emit_no_12e_false_positive() {
    assert_clean(&validate_value(vec![], "plain literal, no tokens"));
}

#[test]
fn arg_present_only_beside_a_bare_flag_is_a_dead_parameter() {
    // A Flag with value: None carries no text, so a declared arg cannot
    // "appear" there — 12d must fire.
    let mut m = base_manifest();
    let mut p = pathway("p", vec![req_arg("key")], "literal");
    p.steps[0].args.push(PathwayArg::Flag {
        name: "--json".to_string(),
        value: None,
    });
    m.pathways = vec![p];
    let result = validate(&m);
    assert!(result.is_valid(), "expected no errors, got: {result}");
    assert_eq!(result.warnings.len(), 1, "expected only 12d: {result}");
    assert!(result.warnings[0].message.contains("appears in no step"));
    assert!(result.warnings[0].message.contains("key"));
}

// ── Round-trip property ──────────────────────────────────────────────────

#[test]
fn rich_pathway_args_round_trip_parse_serialize_parse() {
    let mut m = base_manifest();
    let mut p = pathway("rich", vec![], "<MODE> <LIMIT>");
    p.args = vec![
        Arg {
            name: "mode".to_string(),
            arg_type: "string".to_string(),
            required: false,
            description: "Mode of operation".to_string(),
            default: Some(serde_json::json!("fast")),
            enum_values: Some(vec!["fast".to_string(), "slow".to_string()]),
            constraints: None,
        },
        Arg {
            name: "limit".to_string(),
            arg_type: "integer".to_string(),
            required: true,
            description: "Upper bound".to_string(),
            default: None,
            enum_values: None,
            constraints: Some(serde_json::json!({"minimum": 1, "maximum": 100})),
        },
    ];
    m.pathways = vec![p];

    // The rich manifest must be semantically clean before pinning round-trip.
    assert_clean(&validate(&m));

    let json = serde_json::to_string_pretty(&m).expect("serialize");
    let reparsed: Manifest = serde_json::from_str(&json).expect("reparse");
    assert_eq!(m, reparsed);

    let json2 = serde_json::to_string_pretty(&reparsed).expect("re-serialize");
    assert_eq!(json, json2, "serialization must be a fixed point");
}

#[test]
fn pathway_without_args_round_trips() {
    let mut m = base_manifest();
    m.pathways = vec![pathway("plain", vec![], "no tokens")];
    let json = serde_json::to_string(&m).expect("serialize");
    let reparsed: Manifest = serde_json::from_str(&json).expect("reparse");
    assert_eq!(m, reparsed);
}
