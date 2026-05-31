# tsukai-manifest

使い (tsukai) — the messenger. A structured CLI manifest format designed for AI agent consumption via MCP.

This crate defines the manifest schema, validation logic, and tier projection system that powers the tsukai ecosystem. Tool authors write manifests describing their CLIs; the tsukai MCP bridge reads those manifests and generates compact, tiered tool definitions for AI agents.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design spec.

## Crates

| Crate | Purpose |
|-------|---------|
| `tsukai-manifest` | Core library — types, validation, projection |
| `tsukai-manifest-cli` | CLI binary — `tsukai-manifest validate` |

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
