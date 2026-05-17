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
        path: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { path } => {
            eprintln!("validate: not yet implemented (path: {path})");
            std::process::exit(1);
        }
    }
}
