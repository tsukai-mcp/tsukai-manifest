//! Semantic validation for manifests.
//!
//! This module will hold validation logic that goes beyond JSON schema
//! structural checks: referential integrity between commands and tiers,
//! error kind references, pathway step validity, and other invariants
//! that the type system alone cannot enforce.
