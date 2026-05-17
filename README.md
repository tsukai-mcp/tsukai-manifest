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

MIT
