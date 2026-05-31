# tsukai-manifest

使い (tsukai) — the messenger. A structured CLI manifest format designed for AI-agent consumption via MCP.

This crate provides the core types, validation, and projection logic for the tsukai
manifest format. A manifest fully describes a CLI tool's commands, arguments, output
shapes, mutation semantics, and error taxonomy so that an MCP bridge can generate
correct, compact tool definitions for AI agents.

## Features

- **Typed model** — strongly-typed Rust representation of a manifest (`Manifest`, `Command`, `Arg`, `Flag`, `OutputSchema`, `Pathway`, ...).
- **JSON Schema generation** — emit the canonical JSON Schema for the manifest format via [`generate_manifest_schema`].
- **Semantic validation** — structural deserialization plus a semantic validation layer producing errors and warnings.
- **Tiered projections** — Tier 0 (discovery overview), Tier 1 (core-command summary), and Tier 2 (full per-command detail) projections, with token estimation.

## Example

```rust
use tsukai_manifest::{validate, project_tier0, Manifest};

let manifest: Manifest = serde_json::from_str(json)?;
let result = validate(&manifest);
if result.is_valid() {
    let tier0 = project_tier0(&manifest);
}
```

See [ARCHITECTURE.md](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/ARCHITECTURE.md) for the full design spec.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/LICENSE-APACHE))
- MIT license ([LICENSE-MIT](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/LICENSE-MIT))

at your option.
