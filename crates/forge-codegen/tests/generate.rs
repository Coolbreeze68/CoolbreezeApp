//! Génération complète : idempotence, préservation du code utilisateur, et
//! exemple `examples/crm` à jour.

use std::fs;
use std::path::{Path, PathBuf};

use forge_codegen::{Error, Options, Report, SNAPSHOT};
use forge_schema::Model;

const CRM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/crm");

fn options() -> Options {
    Options {
        runtime_path: "../../../crates/forge-runtime".into(),
    }
}

fn generate(project: &Path) -> Result<Report, Error> {
    let source = fs::read_to_string(project.join("forge.json")).unwrap();
    let model = Model::from_json(&source).unwrap();
    forge_codegen::generate(project, &source, &model, &options())
}

/// Copie un projet, sans ses artefacts de compilation.
fn copy_project(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap();
        if name == "target" {
            continue;
        }
        let dest = to.join(name);
        if path.is_dir() {
            fs::create_dir_all(&dest).unwrap();
            copy_project(&path, &dest);
        } else {
            fs::copy(&path, &dest).unwrap();
        }
    }
}

#[test]
fn crm_example_is_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    copy_project(Path::new(CRM), dir.path());
    let report = generate(dir.path()).unwrap();
    assert!(
        report.is_unchanged(),
        "examples/crm est obsolète, lancez `cargo run -p forge-cli -- generate --dir examples/crm` : {report:?}"
    );
}

#[test]
fn generation_is_idempotent_and_preserves_custom_code() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    fs::copy(
        Path::new(CRM).join("forge.json"),
        project.join("forge.json"),
    )
    .unwrap();

    let first = generate(project).unwrap();
    assert!(
        first
            .created
            .contains(&PathBuf::from("backend/src/migrations/m0001_init.rs"))
    );
    assert!(first.created.contains(&PathBuf::from(SNAPSHOT)));
    assert!(
        generate(project).unwrap().is_unchanged(),
        "deuxième génération"
    );

    // Le code utilisateur survit ; le code généré modifié est restauré.
    let hooks = project.join("backend/src/custom/hooks/contact.rs");
    fs::write(&hooks, "// mon code").unwrap();
    let entity = project.join("backend/src/generated/entities/contact.rs");
    fs::write(&entity, "// modifié à la main").unwrap();
    let report = generate(project).unwrap();
    assert_eq!(
        report.updated,
        [PathBuf::from("backend/src/generated/entities/contact.rs")]
    );
    assert_eq!(fs::read_to_string(&hooks).unwrap(), "// mon code");
}

#[test]
fn removed_table_cleans_generated_code() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    let mut schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(Path::new(CRM).join("forge.json")).unwrap())
            .unwrap();
    fs::write(project.join("forge.json"), schema.to_string()).unwrap();
    generate(project).unwrap();

    // Retirer `tag` change le stockage : refusé tant que les migrations incrémentales
    // (phase 2) n'existent pas, sans rien écrire.
    let tables = schema["tables"].as_array_mut().unwrap();
    tables.retain(|t| t["name"] != "tag");
    for table in tables.iter_mut() {
        table["columns"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c["target"] != "tag");
        if let Some(columns) = table["columns"].as_array_mut() {
            columns.retain(|c| c["name"] != "nb_opportunites");
        }
    }
    fs::write(project.join("forge.json"), schema.to_string()).unwrap();
    let err = generate(project).unwrap_err();
    assert!(matches!(err, Error::StorageChanged { .. }), "{err}");
    assert!(
        project
            .join("backend/src/generated/entities/tag.rs")
            .exists()
    );

    // Projet recréé : plus d'entité `tag`, hooks conservés mais signalés.
    fs::remove_file(project.join(SNAPSHOT)).unwrap();
    fs::remove_dir_all(project.join("backend/src/migrations")).unwrap();
    let report = generate(project).unwrap();
    assert!(
        report
            .deleted
            .contains(&PathBuf::from("backend/src/generated/entities/tag.rs"))
    );
    assert!(project.join("backend/src/custom/hooks/tag.rs").exists());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("custom/hooks/tag.rs")),
        "{:?}",
        report.warnings
    );
}
