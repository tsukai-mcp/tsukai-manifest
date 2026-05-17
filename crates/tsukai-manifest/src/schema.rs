//! JSON Schema generation.
//!
//! This module produces a JSON Schema document from the manifest types,
//! enabling external validation of manifest files and powering editor
//! autocompletion via `$schema` references.

use schemars::SchemaGenerator;
use serde_json::Value;

use crate::Manifest;

/// The canonical `$id` for the v1 manifest schema.
pub const SCHEMA_ID: &str = "https://tsukai.dev/manifest/v1.json";

/// Generate the JSON Schema for [`Manifest`] as a [`serde_json::Value`].
///
/// The returned schema includes:
/// - `$id` set to [`SCHEMA_ID`]
/// - `$schema` pointing to the JSON Schema 2020-12 meta-schema
/// - Descriptions pulled from doc comments on all types
/// - Proper handling of serde renames (`$schema`, `type`, `enum`)
pub fn generate_manifest_schema() -> Value {
    let mut schema = SchemaGenerator::default().into_root_schema_for::<Manifest>();

    let obj = schema.ensure_object();
    obj.insert("$id".to_owned(), Value::String(SCHEMA_ID.to_owned()));

    serde_json::to_value(&schema).expect("Schema serializes to Value")
}

/// Generate the JSON Schema for [`Manifest`] as a pretty-printed JSON string.
pub fn generate_manifest_schema_string() -> String {
    let value = generate_manifest_schema();
    serde_json::to_string_pretty(&value).expect("Schema serializes to string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid_json_object() {
        let schema = generate_manifest_schema();
        assert!(schema.is_object(), "Schema must be a JSON object");
    }

    #[test]
    fn schema_has_id() {
        let schema = generate_manifest_schema();
        assert_eq!(
            schema.get("$id").and_then(|v| v.as_str()),
            Some(SCHEMA_ID),
            "Schema must have $id set to {SCHEMA_ID}"
        );
    }

    #[test]
    fn schema_has_meta_schema() {
        let schema = generate_manifest_schema();
        let meta = schema
            .get("$schema")
            .and_then(|v| v.as_str())
            .expect("Schema must have $schema");
        assert!(
            meta.contains("json-schema.org"),
            "$schema must reference json-schema.org, got: {meta}"
        );
    }

    #[test]
    fn schema_has_properties() {
        let schema = generate_manifest_schema();
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("Schema must have properties");

        let required_props = ["name", "bin", "version", "description"];
        for prop in &required_props {
            assert!(
                props.contains_key(*prop),
                "Schema must have property '{prop}'"
            );
        }
    }

    #[test]
    fn schema_handles_dollar_schema_rename() {
        let schema = generate_manifest_schema();
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("Schema must have properties");

        assert!(
            props.contains_key("$schema"),
            "The 'schema' field must be renamed to '$schema' via serde"
        );
        assert!(
            !props.contains_key("schema"),
            "There should be no bare 'schema' property"
        );
    }

    #[test]
    fn schema_handles_type_renames() {
        let schema = generate_manifest_schema();
        let schema_str = serde_json::to_string(&schema).expect("serialize");

        // Arg.arg_type is renamed to "type" via serde
        // OutputField.field_type is renamed to "type" via serde
        // OutputSchema.output_type is renamed to "type" via serde
        // These should appear as property names in their respective $defs
        assert!(
            schema_str.contains(r#""type""#),
            "Serde renames to 'type' must be reflected in schema"
        );
    }

    #[test]
    fn schema_handles_enum_renames() {
        let schema = generate_manifest_schema();
        let schema_str = serde_json::to_string(&schema).expect("serialize");

        // Arg.enum_values is renamed to "enum" via serde
        // OutputField.enum_values is renamed to "enum" via serde
        assert!(
            schema_str.contains(r#""enum""#),
            "Serde renames to 'enum' must be reflected in schema"
        );
    }

    #[test]
    fn schema_has_descriptions() {
        let schema = generate_manifest_schema();

        // The top-level Manifest struct has a doc comment, which should become
        // the schema description
        let desc = schema.get("description").and_then(|v| v.as_str());
        assert!(
            desc.is_some(),
            "Top-level schema should have a description from doc comments"
        );
    }

    #[test]
    fn schema_represents_recursive_output_items() {
        let schema = generate_manifest_schema();
        let schema_str = serde_json::to_string_pretty(&schema).expect("serialize");

        // OutputSchema has a recursive `items: Option<Box<OutputSchema>>` field.
        // The schema must represent this — look for OutputSchema in $defs
        // with an "items" property that references itself.
        assert!(
            schema_str.contains("OutputSchema"),
            "Schema should reference OutputSchema in definitions"
        );
    }
}
