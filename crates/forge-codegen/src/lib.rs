//! Génération des applications forge.
//!
//! [`generate`] produit les fichiers d'un projet à partir de son schéma validé :
//! - `backend/src/generated/` est réécrit à chaque fois (fichiers obsolètes supprimés) ;
//! - le code utilisateur (`src/custom/`, `main.rs`, `Cargo.toml`) n'est créé qu'une fois ;
//! - une migration n'est créée que si la structure de stockage a changé
//!   (comparée à `.forge/snapshot.json`).
//!
//! Deux générations successives ne produisent aucun changement.

mod backend;
mod error;
mod layout;
mod render;
mod writer;

use std::fs;
use std::path::{Path, PathBuf};

use forge_schema::Model;

pub use error::Error;
pub use layout::Layout;
pub use writer::{OutputFile, Policy, Report};

use crate::backend::Backend;
use crate::render::Renderer;

/// Fichier d'état : structure de stockage à la dernière migration générée.
pub const SNAPSHOT: &str = ".forge/snapshot.json";
const MIGRATIONS_DIR: &str = "backend/src/migrations";

#[derive(Debug, Clone)]
pub struct Options {
    /// Chemin de la crate `forge-runtime`, relatif au dossier `backend/` du projet.
    pub runtime_path: String,
}

/// Génère (ou met à jour) le projet situé dans `project`.
///
/// `schema_source` est le contenu de `forge.json`, déjà validé en `model`.
pub fn generate(
    project: &Path,
    schema_source: &str,
    model: &Model,
    options: &Options,
) -> Result<Report, Error> {
    let renderer = Renderer::new();
    let mut report = Report::default();
    if !renderer.has_rustfmt() {
        report
            .warnings
            .push("rustfmt introuvable : le code généré n'est pas formaté".into());
    }

    let layout = Layout::of(model);
    let mut migrations = existing_migrations(project)?;
    let mut files = Vec::new();
    if let Some((name, migration)) = plan_migration(project, &layout, &migrations, &renderer)? {
        migrations.push(name);
        files.push(migration);
        files.push(OutputFile {
            path: PathBuf::from(SNAPSHOT),
            content: snapshot_json(&layout),
            policy: Policy::Generated,
        });
    }

    let backend = Backend {
        crate_name: &model.spec().app.name,
        schema_source,
        layout: &layout,
        runtime_path: &options.runtime_path,
        migrations: &migrations,
    };
    files.extend(backend.files(&renderer)?);
    writer::write(project, &files, backend::GENERATED_DIRS, &mut report)?;

    report.warnings.extend(check_dependencies(project));
    for table in orphan_hooks(project, model)? {
        report.warnings.push(format!(
            "backend/src/custom/hooks/{table}.rs : la table `{table}` n'existe plus, ce fichier n'est plus compilé"
        ));
    }
    Ok(report)
}

/// Versions attendues par le code généré ; `backend/Cargo.toml` appartient à
/// l'utilisateur, on se contente donc d'avertir en cas d'écart.
const EXPECTED_DEPENDENCIES: &[(&str, &str)] = &[
    ("axum", "0.8"),
    ("sea-orm", "2"),
    ("sea-orm-migration", "2"),
];

fn check_dependencies(project: &Path) -> Vec<String> {
    let path = project.join("backend/Cargo.toml");
    let Ok(manifest) = fs::read_to_string(&path).map(|s| s.parse::<toml::Table>()) else {
        return vec![format!("{} illisible", path.display())];
    };
    let Ok(manifest) = manifest else {
        return vec![format!("{} : TOML invalide", path.display())];
    };
    let dependencies = manifest.get("dependencies").and_then(toml::Value::as_table);
    let version = |name: &str| {
        let dependency = dependencies?.get(name)?;
        dependency
            .as_str()
            .or_else(|| dependency.get("version")?.as_str())
            .map(str::to_owned)
    };
    let mut warnings = Vec::new();
    if dependencies.and_then(|d| d.get("forge-runtime")).is_none() {
        warnings.push("backend/Cargo.toml : dépendance `forge-runtime` absente".to_owned());
    }
    for (name, expected) in EXPECTED_DEPENDENCIES {
        match version(name) {
            Some(found) if found == *expected => {}
            found => warnings.push(format!(
                "backend/Cargo.toml : `{name}` en version {}, le code généré attend {expected}",
                found.as_deref().unwrap_or("absente")
            )),
        }
    }
    warnings
}

fn snapshot_json(layout: &Layout) -> String {
    serde_json::to_string_pretty(layout).expect("sérialisation du layout") + "\n"
}

/// Migration à créer si la structure de stockage a changé depuis le snapshot.
fn plan_migration(
    project: &Path,
    layout: &Layout,
    migrations: &[String],
    renderer: &Renderer,
) -> Result<Option<(String, OutputFile)>, Error> {
    let path = project.join(SNAPSHOT);
    let previous = match fs::read_to_string(&path) {
        Ok(content) => {
            Some(
                serde_json::from_str::<Layout>(&content).map_err(|err| Error::Snapshot {
                    path: path.clone(),
                    message: err.to_string(),
                })?,
            )
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(Error::io(&path)(err)),
    };
    match previous {
        None if migrations.is_empty() => {
            let name = "m0001_init".to_owned();
            let file = backend::initial_migration(renderer, &name, layout)?;
            Ok(Some((name, file)))
        }
        None => Err(Error::Snapshot {
            path,
            message: "fichier manquant alors que des migrations existent".into(),
        }),
        Some(previous) if previous == *layout => Ok(None),
        Some(previous) => Err(Error::StorageChanged {
            changes: summarize_changes(&previous, layout),
        }),
    }
}

fn summarize_changes(previous: &Layout, current: &Layout) -> String {
    let mut tables: Vec<&str> = current
        .tables
        .iter()
        .filter(|t| !previous.tables.contains(t))
        .chain(
            previous
                .tables
                .iter()
                .filter(|t| !current.tables.contains(t)),
        )
        .map(|t| t.name.as_str())
        .collect();
    tables.sort_unstable();
    tables.dedup();
    if previous.join_tables != current.join_tables {
        tables.push("tables de jointure");
    }
    format!("concerne : {}", tables.join(", "))
}

/// Migrations présentes dans `backend/src/migrations/`, triées.
fn existing_migrations(project: &Path) -> Result<Vec<String>, Error> {
    let dir = project.join(MIGRATIONS_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(Error::io(&dir)(err)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let path = entry.map_err(Error::io(&dir))?.path();
        if path.extension().is_some_and(|e| e == "rs") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Fichiers de hooks dont la table a disparu du schéma.
fn orphan_hooks(project: &Path, model: &Model) -> Result<Vec<String>, Error> {
    let dir = project.join("backend/src/custom/hooks");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut orphans = Vec::new();
    for entry in entries {
        let path = entry.map_err(Error::io(&dir))?.path();
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if model.table(stem).is_none() {
                orphans.push(stem.to_owned());
            }
        }
    }
    orphans.sort();
    Ok(orphans)
}
