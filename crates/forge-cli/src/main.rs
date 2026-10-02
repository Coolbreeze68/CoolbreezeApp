//! `forge` : générateur d'applications à partir d'un schéma JSON.

use std::path::{Component, Path, PathBuf};
use std::process::{Command as Process, ExitCode};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use forge_codegen::Options;
use forge_schema::{Model, SchemaError};

/// Emplacement de `forge-runtime` dans les sources de forge, utilisé par défaut.
const RUNTIME_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../forge-runtime");

#[derive(Debug, Parser)]
#[command(name = "forge", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Crée un projet à partir d'un schéma, puis génère son code.
    New {
        /// Dossier du projet (créé, doit être absent ou vide).
        dir: PathBuf,
        /// Schéma de départ, copié dans `<dir>/forge.json`.
        #[arg(long)]
        schema: PathBuf,
        /// Chemin de la crate `forge-runtime` (par défaut : celle des sources de forge).
        #[arg(long)]
        runtime_path: Option<PathBuf>,
    },
    /// Régénère le code d'un projet à partir de son `forge.json`.
    Generate {
        /// Dossier du projet.
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Chemin de la crate `forge-runtime` (utilisé à la création de `backend/Cargo.toml`).
        #[arg(long)]
        runtime_path: Option<PathBuf>,
    },
    /// Applique ou annule les migrations (via `cargo run` dans `backend/`).
    Migrate {
        #[arg(value_enum, default_value_t = MigrateAction::Up)]
        action: MigrateAction,
        /// Dossier du projet.
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Valide un schéma et affiche toutes les erreurs trouvées.
    Validate {
        /// Fichier de schéma.
        #[arg(default_value = "forge.json")]
        schema: PathBuf,
    },
    /// Affiche le JSON Schema du format d'entrée (contenu de `forge.schema.json`).
    Schema,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MigrateAction {
    /// Applique les migrations en attente.
    Up,
    /// Annule la dernière migration.
    Down,
    /// Affiche l'état des migrations.
    Status,
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
        Command::New {
            dir,
            schema,
            runtime_path,
        } => new(&dir, &schema, runtime_path.as_deref()),
        Command::Generate { dir, runtime_path } => generate(&dir, runtime_path.as_deref()),
        Command::Migrate { action, dir } => migrate(&dir, action),
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

/// Lit et valide un schéma ; affiche les erreurs et retourne `None` s'il est invalide.
fn load(path: &Path) -> anyhow::Result<Option<(String, Model)>> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("lecture de `{}`", path.display()))?;
    match Model::from_json(&src) {
        Ok(model) => Ok(Some((src, model))),
        Err(SchemaError { issues }) => {
            eprintln!("{} : {} erreur(s)", path.display(), issues.len());
            for issue in issues {
                eprintln!("  - {issue}");
            }
            Ok(None)
        }
    }
}

fn validate(path: &Path) -> anyhow::Result<ExitCode> {
    let Some((_, model)) = load(path)? else {
        return Ok(ExitCode::FAILURE);
    };
    println!(
        "{} : schéma valide ({} table(s), {} colonne(s) calculée(s))",
        path.display(),
        model.tables().len(),
        model.computed_order().len()
    );
    Ok(ExitCode::SUCCESS)
}

fn new(dir: &Path, schema: &Path, runtime_path: Option<&Path>) -> anyhow::Result<ExitCode> {
    if dir.exists() && dir.read_dir()?.next().is_some() {
        bail!("`{}` existe et n'est pas vide", dir.display());
    }
    // Validation avant toute écriture.
    if load(schema)?.is_none() {
        return Ok(ExitCode::FAILURE);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("création de `{}`", dir.display()))?;
    std::fs::copy(schema, dir.join("forge.json")).context("copie du schéma")?;
    let code = generate(dir, runtime_path)?;
    if code == ExitCode::SUCCESS {
        println!(
            "\nProjet créé. Pour démarrer l'API :\n  cd {}/backend && cargo run",
            dir.display()
        );
    }
    Ok(code)
}

fn generate(dir: &Path, runtime_path: Option<&Path>) -> anyhow::Result<ExitCode> {
    let Some((source, model)) = load(&dir.join("forge.json"))? else {
        return Ok(ExitCode::FAILURE);
    };
    let runtime = runtime_path.unwrap_or(Path::new(RUNTIME_PATH));
    let runtime = runtime
        .canonicalize()
        .with_context(|| format!("forge-runtime introuvable : `{}`", runtime.display()))?;
    let backend = dir.canonicalize()?.join("backend");
    let options = Options {
        runtime_path: relative_path(&backend, &runtime).display().to_string(),
    };

    let report = forge_codegen::generate(dir, &source, &model, &options)?;
    for (label, paths) in [
        ("créé", &report.created),
        ("mis à jour", &report.updated),
        ("supprimé", &report.deleted),
    ] {
        for path in paths {
            println!("  {label:<10} {}", path.display());
        }
    }
    for warning in &report.warnings {
        eprintln!("attention : {warning}");
    }
    if report.is_unchanged() {
        println!("Aucun changement.");
    }
    Ok(ExitCode::SUCCESS)
}

fn migrate(dir: &Path, action: MigrateAction) -> anyhow::Result<ExitCode> {
    let manifest = dir.join("backend/Cargo.toml");
    if !manifest.exists() {
        bail!(
            "`{}` introuvable : lancez d'abord `forge generate`",
            manifest.display()
        );
    }
    let action = match action {
        MigrateAction::Up => "up",
        MigrateAction::Down => "down",
        MigrateAction::Status => "status",
    };
    let status = Process::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .args(["--", "migrate", action])
        .status()
        .context("lancement de cargo")?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Chemin de `target` relatif à `base` (tous deux absolus), ou `target` lui-même
/// s'ils n'ont en commun que la racine.
fn relative_path(base: &Path, target: &Path) -> PathBuf {
    let base: Vec<Component> = base.components().collect();
    let target_parts: Vec<Component> = target.components().collect();
    let common = base
        .iter()
        .zip(&target_parts)
        .take_while(|(a, b)| a == b)
        .count();
    if common <= 1 {
        return target.to_path_buf();
    }
    let mut path: PathBuf =
        std::iter::repeat_n(Component::ParentDir, base.len() - common).collect();
    path.extend(&target_parts[common..]);
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative_path(
                Path::new("/a/b/crm/backend"),
                Path::new("/a/forge/crates/forge-runtime")
            ),
            PathBuf::from("../../../forge/crates/forge-runtime")
        );
        assert_eq!(
            relative_path(Path::new("/a"), Path::new("/a/b")),
            PathBuf::from("b")
        );
    }
}
