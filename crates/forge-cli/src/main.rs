//! `forge` : générateur d'applications à partir d'un schéma JSON.

use std::path::{Component, Path, PathBuf};
use std::process::{Command as Process, ExitCode};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use forge_codegen::Options;
use forge_schema::{Model, SchemaError};

/// Emplacement de `forge-runtime` dans les sources de forge, utilisé par défaut.
const RUNTIME_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../forge-runtime");

/// Emplacement de `forge_flutter` dans les sources de forge, utilisé par défaut.
const FLUTTER_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/forge_flutter");

/// Bibliothèques dont dépendent les projets générés (utilisées à la création
/// de `backend/Cargo.toml` et `app/pubspec.yaml`).
#[derive(Debug, Args)]
struct Libraries {
    /// Chemin de la crate `forge-runtime` (par défaut : celle des sources de forge).
    #[arg(long)]
    runtime_path: Option<PathBuf>,
    /// Chemin du package `forge_flutter` (par défaut : celui des sources de forge).
    #[arg(long)]
    flutter_path: Option<PathBuf>,
}

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
        #[command(flatten)]
        libraries: Libraries,
    },
    /// Régénère le code d'un projet à partir de son `forge.json`.
    Generate {
        /// Dossier du projet.
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[command(flatten)]
        libraries: Libraries,
        /// Génère la migration même si elle supprime ou convertit des données.
        #[arg(long)]
        allow_destructive: bool,
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
            libraries,
        } => new(&dir, &schema, &libraries),
        Command::Generate {
            dir,
            libraries,
            allow_destructive,
        } => generate(&dir, &libraries, allow_destructive),
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

fn new(dir: &Path, schema: &Path, libraries: &Libraries) -> anyhow::Result<ExitCode> {
    if dir.exists() && dir.read_dir()?.next().is_some() {
        bail!("`{}` existe et n'est pas vide", dir.display());
    }
    // Validation avant toute écriture.
    if load(schema)?.is_none() {
        return Ok(ExitCode::FAILURE);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("création de `{}`", dir.display()))?;
    std::fs::copy(schema, dir.join("forge.json")).context("copie du schéma")?;
    let code = generate(dir, libraries, false)?;
    if code == ExitCode::SUCCESS {
        println!(
            "\nProjet créé.\n  API :         cd {0}/backend && cargo run\n  \
             Application : cd {0}/app && flutter run -d chrome \
             --dart-define=FORGE_API_URL=http://localhost:8080",
            dir.display()
        );
    }
    Ok(code)
}

fn generate(
    dir: &Path,
    libraries: &Libraries,
    allow_destructive: bool,
) -> anyhow::Result<ExitCode> {
    let Some((source, model)) = load(&dir.join("forge.json"))? else {
        return Ok(ExitCode::FAILURE);
    };
    let locate = |path: Option<&PathBuf>, default: &str, name: &str| {
        let path = path.map_or(Path::new(default), PathBuf::as_path);
        path.canonicalize()
            .with_context(|| format!("{name} introuvable : `{}`", path.display()))
    };
    let runtime = locate(
        libraries.runtime_path.as_ref(),
        RUNTIME_PATH,
        "forge-runtime",
    )?;
    let flutter = locate(
        libraries.flutter_path.as_ref(),
        FLUTTER_PATH,
        "forge_flutter",
    )?;
    let project = dir.canonicalize()?;
    // Racine des sources de forge (`crates/forge-runtime` en est à deux niveaux) :
    // contexte de construction de l'image Docker.
    let forge_root = runtime.parent().and_then(Path::parent).unwrap_or(&runtime);
    let options = Options {
        runtime_path: relative_path(&project.join("backend"), &runtime)
            .display()
            .to_string(),
        flutter_path: relative_path(&project.join("app"), &flutter)
            .display()
            .to_string(),
        web_path: relative_path(&project.join("web"), &forge_root.join("packages/forge_web"))
            .display()
            .to_string(),
        forge_path: relative_path(&project, forge_root).display().to_string(),
        allow_destructive,
    };

    let report = forge_codegen::generate(dir, &source, &model, &options)?;
    if let Some((name, changes)) = &report.migration {
        println!("Migration {name} :");
        for change in changes {
            println!("  - {change}");
        }
        println!("Appliquez-la avec `forge migrate`.\n");
    }
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
