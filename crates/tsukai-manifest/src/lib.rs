//! tsukai-manifest — structured CLI manifest format for AI agent consumption via MCP.
//!
//! This crate provides the core types, validation, and projection logic for the
//! tsukai manifest format. A manifest fully describes a CLI tool's commands,
//! arguments, output shapes, mutation semantics, and error taxonomy so that an
//! MCP bridge can generate correct tool definitions for AI agents.

pub mod manifest;
pub mod projection;
pub mod schema;
pub mod validation;

pub use manifest::{
    AgentConfig, Arg, Command, ContextRequirements, ErrorDef, Flag, Manifest, OutputField,
    OutputSchema, Pathway, PathwayStep, Tiers,
};
pub use projection::{
    CommandGroupSummary, CoreCommandSummary, ErrorDetail, Tier0, Tier1, Tier2Arg, Tier2Command,
    Tier2Flag, estimate_tokens, project_tier0, project_tier1, project_tier2_command,
};
pub use schema::{SCHEMA_ID, generate_manifest_schema, generate_manifest_schema_string};
pub use validation::{ValidationError, ValidationResult, ValidationWarning, validate};
