//! Staleness check for the generated JSON Schema.
//!
//! This test ensures `schemas/manifest.v1.json` stays in sync with the Rust
//! types. If someone changes the types but forgets to regenerate the schema,
//! this test fails with a diff showing exactly what changed.
//!
//! To update the on-disk schema after changing types:
//!
//! ```sh
//! UPDATE_SCHEMA=1 cargo test -p tsukai-manifest schema_staleness
//! ```
//!
//! Or use the CLI:
//!
//! ```sh
//! cargo run -p tsukai-manifest-cli -- schema -o schemas/manifest.v1.json
//! ```

use std::path::PathBuf;

fn schema_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Go up from crates/tsukai-manifest to workspace root
    path.pop();
    path.pop();
    path.push("schemas");
    path.push("manifest.v1.json");
    path
}

#[test]
fn schema_not_stale() {
    let generated = tsukai_manifest::generate_manifest_schema_string();
    let generated_with_newline = format!("{generated}\n");

    let path = schema_path();

    if std::env::var("UPDATE_SCHEMA").is_ok() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create schemas directory");
        }
        std::fs::write(&path, &generated_with_newline).expect("write schema");
        eprintln!("Updated schema at {}", path.display());
        return;
    }

    let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to read {}: {e}\n\n\
             Hint: Generate the schema with:\n  \
             UPDATE_SCHEMA=1 cargo test -p tsukai-manifest schema_staleness\n  \
             or: cargo run -p tsukai-manifest-cli -- schema -o schemas/manifest.v1.json",
            path.display()
        );
    });

    if on_disk != generated_with_newline {
        // Parse both as Values to give a useful diff
        let on_disk_value: serde_json::Value =
            serde_json::from_str(&on_disk).expect("on-disk schema is valid JSON");
        let generated_value: serde_json::Value =
            serde_json::from_str(&generated).expect("generated schema is valid JSON");

        panic!(
            "Schema is stale! The on-disk schema at {} does not match \
             what the current types generate.\n\n\
             On-disk:\n{}\n\n\
             Generated:\n{}\n\n\
             To update, run:\n  \
             UPDATE_SCHEMA=1 cargo test -p tsukai-manifest schema_staleness\n  \
             or: cargo run -p tsukai-manifest-cli -- schema -o schemas/manifest.v1.json",
            path.display(),
            serde_json::to_string_pretty(&on_disk_value).unwrap(),
            serde_json::to_string_pretty(&generated_value).unwrap(),
        );
    }
}
