use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// CLI for validating and inspecting tsukai manifests.
#[derive(Parser)]
#[command(name = "tsukai-manifest", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a tsukai manifest file
    Validate {
        /// Path to the manifest file
        path: PathBuf,
    },

    /// Generate the JSON Schema for the manifest format
    GenerateSchema {
        /// Output path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { path } => {
            eprintln!("validate: not yet implemented (path: {})", path.display());
            std::process::exit(1);
        }
        Commands::GenerateSchema { output } => {
            let schema = tsukai_manifest::generate_manifest_schema_string();

            match output {
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                            eprintln!("Failed to create directory {}: {e}", parent.display());
                            std::process::exit(1);
                        });
                    }
                    std::fs::write(&path, format!("{schema}\n")).unwrap_or_else(|e| {
                        eprintln!("Failed to write {}: {e}", path.display());
                        std::process::exit(1);
                    });
                    eprintln!("Schema written to {}", path.display());
                }
                None => {
                    println!("{schema}");
                }
            }
        }
    }
}
