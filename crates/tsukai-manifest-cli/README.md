# tsukai-manifest-cli

Command-line interface for the [tsukai manifest format](https://crates.io/crates/tsukai-manifest).

Installs the `tsukai-manifest` binary.

## Install

```sh
cargo install tsukai-manifest-cli
```

## Usage

```sh
# Validate a manifest (semantic + structural checks)
tsukai-manifest validate path/to/manifest.json

# Project a manifest into a tier
tsukai-manifest project path/to/manifest.json --tier 0
tsukai-manifest project path/to/manifest.json --tier 1
tsukai-manifest project path/to/manifest.json --tier 2 --command <name>

# Emit the JSON Schema for the manifest format
tsukai-manifest schema -o manifest.schema.json
```

A global `--json` flag switches every subcommand to machine-readable output.

See [ARCHITECTURE.md](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/ARCHITECTURE.md) for the full design spec.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/LICENSE-APACHE))
- MIT license ([LICENSE-MIT](https://github.com/tsukai-mcp/tsukai-manifest/blob/main/LICENSE-MIT))

at your option.
