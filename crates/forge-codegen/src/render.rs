//! Moteur de templates (embarqués dans le binaire) et formatage du code Rust.

use std::io::Write;
use std::process::{Command, Stdio};

use minijinja::Environment;
use serde::Serialize;

use crate::error::Error;

/// Templates embarqués : (nom, contenu).
const TEMPLATES: &[(&str, &str)] = &[
    (
        "backend/cargo.toml",
        include_str!("../../../templates/backend/cargo.toml.j2"),
    ),
    (
        "backend/crud_test.rs",
        include_str!("../../../templates/backend/crud_test.rs.j2"),
    ),
    (
        "backend/custom_functions.rs",
        include_str!("../../../templates/backend/custom_functions.rs.j2"),
    ),
    (
        "backend/custom_hooks.rs",
        include_str!("../../../templates/backend/custom_hooks.rs.j2"),
    ),
    (
        "backend/custom_mod.rs",
        include_str!("../../../templates/backend/custom_mod.rs.j2"),
    ),
    (
        "backend/custom_routes.rs",
        include_str!("../../../templates/backend/custom_routes.rs.j2"),
    ),
    (
        "backend/entities_mod.rs",
        include_str!("../../../templates/backend/entities_mod.rs.j2"),
    ),
    (
        "backend/entity.rs",
        include_str!("../../../templates/backend/entity.rs.j2"),
    ),
    (
        "backend/generated_mod.rs",
        include_str!("../../../templates/backend/generated_mod.rs.j2"),
    ),
    (
        "backend/gitignore",
        include_str!("../../../templates/backend/gitignore.j2"),
    ),
    (
        "backend/hooks_mod.rs",
        include_str!("../../../templates/backend/hooks_mod.rs.j2"),
    ),
    (
        "backend/lib.rs",
        include_str!("../../../templates/backend/lib.rs.j2"),
    ),
    (
        "backend/main.rs",
        include_str!("../../../templates/backend/main.rs.j2"),
    ),
    (
        "backend/migration.rs",
        include_str!("../../../templates/backend/migration.rs.j2"),
    ),
    (
        "backend/migrations_mod.rs",
        include_str!("../../../templates/backend/migrations_mod.rs.j2"),
    ),
];

pub(crate) struct Renderer {
    env: Environment<'static>,
    /// `rustfmt` est-il disponible ? Sinon le code reste valide mais non formaté.
    rustfmt: bool,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        let mut env = Environment::new();
        env.set_keep_trailing_newline(true);
        for (name, source) in TEMPLATES {
            env.add_template(name, source)
                .unwrap_or_else(|err| panic!("template `{name}` invalide : {err}"));
        }
        Self {
            env,
            rustfmt: Command::new("rustfmt").arg("--version").output().is_ok(),
        }
    }

    pub(crate) fn has_rustfmt(&self) -> bool {
        self.rustfmt
    }

    /// Rend un template ; les fichiers `.rs` sont passés par `rustfmt`.
    pub(crate) fn render(&self, template: &str, context: impl Serialize) -> Result<String, Error> {
        let source = self
            .env
            .get_template(template)
            .and_then(|t| t.render(context))
            .map_err(|err| Error::Template(format!("{template} : {err:#}")))?;
        if self.rustfmt
            && std::path::Path::new(template)
                .extension()
                .is_some_and(|e| e == "rs")
        {
            rustfmt(&source).map_err(|err| Error::Template(format!("{template} : {err}")))
        } else {
            Ok(source)
        }
    }
}

fn rustfmt(source: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("lancement de rustfmt : {err}"))?;
    child
        .stdin
        .take()
        .expect("stdin capturé")
        .write_all(source.as_bytes())
        .map_err(|err| format!("écriture vers rustfmt : {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("rustfmt : {err}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|err| err.to_string())
    } else {
        Err(format!(
            "code Rust généré invalide :\n{}\n--- source ---\n{source}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
