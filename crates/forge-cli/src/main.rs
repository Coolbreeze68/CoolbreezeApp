//! `forge` : générateur d'applications à partir d'un schéma JSON.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use forge_schema::{Model, SchemaError};

#[derive(Debug, Parser)]
#[command(name = "forge", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Valide un schéma et affiche toutes les erreurs trouvées.
    Validate {
        /// Fichier de schéma.
        #[arg(default_value = "forge.json")]
        schema: PathBuf,
    },
    /// Affiche le JSON Schema du format d'entrée (contenu de `forge.schema.json`).
    Schema,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("erreur : {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Validate { schema } => validate(&schema),
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&forge_schema::json_schema())?
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn validate(path: &Path) -> anyhow::Result<ExitCode> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("lecture de `{}`", path.display()))?;
    match Model::from_json(&src) {
        Ok(model) => {
            let tables = model.tables().len();
            let computed = model.computed_order().len();
            println!(
                "{} : schéma valide ({tables} table(s), {computed} colonne(s) calculée(s))",
                path.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(SchemaError { issues }) => {
            eprintln!("{} : {} erreur(s)", path.display(), issues.len());
            for issue in issues {
                eprintln!("  - {issue}");
            }
            Ok(ExitCode::FAILURE)
        }
    }
}
