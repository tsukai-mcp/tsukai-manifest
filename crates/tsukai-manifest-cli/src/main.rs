//! `tsukai-manifest` — command-line interface for validating and inspecting
//! tsukai manifests.
//!
//! Subcommands:
//! - `validate <path>` — parse + semantically validate a manifest.
//! - `project <path> --tier <0|1|2> [--command <name>]` — emit a tier projection.
//! - `schema [-o <path>]` — emit the JSON Schema for the manifest format.
//!
//! A global `--json` flag switches every subcommand to machine-readable output.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use tsukai_manifest::{
    Manifest, ValidationResult, project_tier0, project_tier1, project_tier2_command, validate,
};

/// CLI for validating and inspecting tsukai manifests.
#[derive(Parser)]
#[command(name = "tsukai-manifest", version, about)]
struct Cli {
    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a tsukai manifest file.
    ///
    /// Parses the file, deserializes it into the manifest model (catching
    /// structural errors), then runs the semantic validation layer. Prints
    /// errors and warnings; exits non-zero only when there are errors
    /// (warnings alone are a success).
    Validate {
        /// Path to the manifest JSON file.
        path: PathBuf,
    },

    /// Emit a tier projection of a manifest as JSON.
    ///
    /// Tier 0 is the discovery overview, tier 1 is the core-command summary,
    /// and tier 2 is the full detail for a single command (requires
    /// `--command`).
    Project {
        /// Path to the manifest JSON file.
        path: PathBuf,

        /// Projection tier to emit.
        #[arg(long, value_enum)]
        tier: Tier,

        /// Command name to project (required for tier 2, ignored otherwise).
        #[arg(long)]
        command: Option<String>,
    },

    /// Print the JSON Schema for the manifest format.
    ///
    /// Output is always JSON. The `--json` flag has no additional effect here.
    Schema {
        /// Write the schema to this path instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Tier {
    #[value(name = "0")]
    Zero,
    #[value(name = "1")]
    One,
    #[value(name = "2")]
    Two,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Validate { path } => run_validate(&path, cli.json),
        Commands::Project {
            path,
            tier,
            command,
        } => run_project(&path, tier, command.as_deref(), cli.json),
        Commands::Schema { output } => run_schema(output.as_deref()),
    };

    match result {
        Ok(code) => code,
        Err(err) => {
            report_cli_error(&err, cli.json);
            ExitCode::FAILURE
        }
    }
}

/// A user-facing CLI error (bad path, malformed JSON, missing argument).
/// Distinct from a manifest *validation* error, which is reported through the
/// validation result rather than this type.
struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn report_cli_error(err: &CliError, json: bool) {
    if json {
        let payload = json!({ "ok": false, "error": err.message });
        // Best-effort: if serialization itself fails we still surface the raw message.
        match serde_json::to_string_pretty(&payload) {
            Ok(s) => eprintln!("{s}"),
            Err(_) => eprintln!("{}", err.message),
        }
    } else {
        eprintln!("{} {}", paint("error:", Color::Red), err.message);
    }
}

/// Read a file and parse it into a [`Manifest`], producing a clean [`CliError`]
/// (never a panic) for missing files, unreadable files, or malformed JSON.
fn load_manifest(path: &Path) -> Result<Manifest, CliError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| CliError::new(format!("failed to read {}: {e}", path.display())))?;

    serde_json::from_str(&contents)
        .map_err(|e| CliError::new(format!("{} is not a valid manifest: {e}", path.display())))
}

fn run_validate(path: &Path, json: bool) -> Result<ExitCode, CliError> {
    let manifest = load_manifest(path)?;
    let result = validate(&manifest);

    if json {
        print_validation_json(path, &result);
    } else {
        print_validation_human(path, &result);
    }

    Ok(if result.is_valid() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn print_validation_json(path: &Path, result: &ValidationResult) {
    let errors: Vec<_> = result
        .errors
        .iter()
        .map(|e| json!({ "path": e.path, "message": e.message }))
        .collect();
    let warnings: Vec<_> = result
        .warnings
        .iter()
        .map(|w| json!({ "path": w.path, "message": w.message }))
        .collect();

    let payload = json!({
        "ok": result.is_valid(),
        "file": path.display().to_string(),
        "errors": errors,
        "warnings": warnings,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("validation payload serializes")
    );
}

fn print_validation_human(path: &Path, result: &ValidationResult) {
    for err in &result.errors {
        println!(
            "{} {}: {}",
            paint("error", Color::Red),
            err.path,
            err.message
        );
    }
    for warn in &result.warnings {
        println!(
            "{} {}: {}",
            paint("warning", Color::Yellow),
            warn.path,
            warn.message
        );
    }

    let file = path.display();
    if result.is_valid() {
        if result.has_warnings() {
            println!(
                "{} {file} is valid ({} warning(s))",
                paint("ok", Color::Green),
                result.warnings.len()
            );
        } else {
            println!("{} {file} is valid", paint("ok", Color::Green));
        }
    } else {
        println!(
            "{} {file} has {} error(s){}",
            paint("invalid", Color::Red),
            result.errors.len(),
            if result.has_warnings() {
                format!(", {} warning(s)", result.warnings.len())
            } else {
                String::new()
            }
        );
    }
}

fn run_project(
    path: &Path,
    tier: Tier,
    command: Option<&str>,
    _json: bool,
) -> Result<ExitCode, CliError> {
    let manifest = load_manifest(path)?;

    // Projections are inherently structured; output is always JSON regardless
    // of the global --json flag.
    let output = match tier {
        Tier::Zero => serde_json::to_string_pretty(&project_tier0(&manifest)),
        Tier::One => serde_json::to_string_pretty(&project_tier1(&manifest)),
        Tier::Two => {
            let name = command
                .ok_or_else(|| CliError::new("tier 2 projection requires --command <name>"))?;
            let projection = project_tier2_command(&manifest, name).ok_or_else(|| {
                CliError::new(format!("command \"{name}\" does not exist in the manifest"))
            })?;
            serde_json::to_string_pretty(&projection)
        }
    }
    .expect("projection serializes to JSON");

    println!("{output}");
    Ok(ExitCode::SUCCESS)
}

fn run_schema(output: Option<&Path>) -> Result<ExitCode, CliError> {
    let schema = tsukai_manifest::generate_manifest_schema_string();

    match output {
        Some(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CliError::new(format!(
                        "failed to create directory {}: {e}",
                        parent.display()
                    ))
                })?;
            }
            std::fs::write(path, format!("{schema}\n"))
                .map_err(|e| CliError::new(format!("failed to write {}: {e}", path.display())))?;
            eprintln!("Schema written to {}", path.display());
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            // Ignore broken-pipe errors (e.g. piping into `head`).
            let _ = writeln!(stdout, "{schema}");
        }
    }

    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// Color output
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum Color {
    Red,
    Yellow,
    Green,
}

impl Color {
    fn code(self) -> &'static str {
        match self {
            Color::Red => "31",
            Color::Yellow => "33",
            Color::Green => "32",
        }
    }
}

/// Wrap `text` in an ANSI color escape when stdout is a terminal; otherwise
/// return it unchanged so piped/redirected output stays clean.
fn paint(text: &str, color: Color) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[1;{}m{text}\x1b[0m", color.code())
    } else {
        text.to_string()
    }
}
