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
