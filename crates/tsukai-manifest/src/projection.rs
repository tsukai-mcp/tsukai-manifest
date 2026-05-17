//! Tier projection logic.
//!
//! This module will generate compact, tiered representations of a manifest
//! for agent consumption:
//!
//! - **Tier 0 (Discovery):** Tool name, description, command groups (~150 tokens)
//! - **Tier 1 (Core Commands):** Key commands with args, return types, mutation flags (~600 tokens)
//! - **Tier 2 (Extended):** Full command details, loaded on demand
//!
//! The bridge uses these projections to manage context window budgets.
