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
        flutter_path: "../../../packages/forge_flutter".into(),
        forge_path: "../..".into(),
        allow_destructive: false,
    }
}

fn generate(project: &Path) -> Result<Report, Error> {
    generate_with(project, &options())
}

fn generate_with(project: &Path, options: &Options) -> Result<Report, Error> {
    let source = fs::read_to_string(project.join("forge.json")).unwrap();
    let model = Model::from_json(&source).unwrap();
    forge_codegen::generate(project, &source, &model, options)
}

fn read_schema(project: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(project.join("forge.json")).unwrap()).unwrap()
}

fn write_schema(project: &Path, schema: &serde_json::Value) {
    fs::write(
        project.join("forge.json"),
        serde_json::to_string_pretty(schema).unwrap(),
    )
    .unwrap();
}

/// Copie un projet, sans ses artefacts de compilation.
fn copy_project(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap();
        if name == "target" || name == ".dart_tool" || name == "build" {
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
fn schema_changes_produce_incremental_migrations() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    fs::copy(
        Path::new(CRM).join("forge.json"),
        project.join("forge.json"),
    )
    .unwrap();
    generate(project).unwrap();
    let hooks = project.join("backend/src/custom/hooks/contact.rs");
    fs::write(&hooks, "// mon code").unwrap();

    // Ajout d'une colonne et renommage d'une autre.
    let mut schema = read_schema(project);
    let contact = &mut schema["tables"][1]["columns"];
    contact
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "name": "linkedin", "type": "string" }));
    contact[3]["name"] = "mobile".into();
    contact[3]["renamed_from"] = "telephone".into();
    write_schema(project, &schema);

    let report = generate(project).unwrap();
    let (name, changes) = report.migration.as_ref().expect("migration");
    assert_eq!(name, "m0002_contact");
    let changes: Vec<String> = changes.iter().map(ToString::to_string).collect();
    assert_eq!(
        changes,
        [
            "`contact` : `telephone` renommée en `mobile`",
            "`contact` : nouvelle colonne `linkedin`"
        ]
    );
    let migration =
        fs::read_to_string(project.join("backend/src/migrations/m0002_contact.rs")).unwrap();
    assert!(
        migration.contains(r#".rename_column("contact", "telephone", "mobile")"#),
        "{migration}"
    );
    assert!(
        migration.contains(r#".add_column("contact", string_null("linkedin"))"#),
        "{migration}"
    );
    assert!(migration.contains(r#".redefine("contact""#), "{migration}");
    assert!(
        migration.contains(r#".rename_column("contact", "mobile", "telephone")"#),
        "retour : {migration}"
    );
    let migrations =
        fs::read_to_string(project.join("backend/src/generated/migrations.rs")).unwrap();
    assert!(migrations.contains("m0002_contact::Migration"));
    let entity =
        fs::read_to_string(project.join("backend/src/generated/entities/contact.rs")).unwrap();
    assert!(entity.contains("pub linkedin: Option<String>"));
    assert_eq!(fs::read_to_string(&hooks).unwrap(), "// mon code");
    assert!(
        generate(project).unwrap().is_unchanged(),
        "migration émise une seule fois"
    );
}

#[test]
fn destructive_changes_require_explicit_permission() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    fs::copy(
        Path::new(CRM).join("forge.json"),
        project.join("forge.json"),
    )
    .unwrap();
    generate(project).unwrap();

    // Suppression d'une table : refusée sans `--allow-destructive`, rien n'est écrit.
    let mut schema = read_schema(project);
    let tables = schema["tables"].as_array_mut().unwrap();
    tables.retain(|t| t["name"] != "tag");
    tables[2]["columns"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["name"] != "tags");
    write_schema(project, &schema);
    let err = generate(project).unwrap_err();
    assert!(
        matches!(&err, Error::Destructive { changes } if changes.len() == 2),
        "{err}"
    );
    assert!(err.to_string().contains("--allow-destructive"));
    assert!(
        !project
            .join("backend/src/migrations/m0002_tag_opportunite.rs")
            .exists()
    );

    let options = Options {
        allow_destructive: true,
        ..options()
    };
    let report = generate_with(project, &options).unwrap();
    assert_eq!(
        report.migration.as_ref().unwrap().0,
        "m0002_tag_opportunite"
    );
    let migration =
        fs::read_to_string(project.join("backend/src/migrations/m0002_tag_opportunite.rs"))
            .unwrap();
    assert!(
        migration.contains("ATTENTION, migration destructive"),
        "{migration}"
    );
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
