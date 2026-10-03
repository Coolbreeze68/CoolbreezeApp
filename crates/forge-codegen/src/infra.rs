//! Déploiement du projet : image Docker, docker-compose, CI GitHub Actions.
//! Fichiers créés une fois, puis à l'utilisateur.

use std::path::PathBuf;

use forge_schema::spec::Frontend;

use crate::error::Error;
use crate::render::Renderer;
use crate::writer::{OutputFile, Policy};

pub(crate) struct Infra<'a> {
    pub app_name: &'a str,
    /// Sources de forge, relatives au dossier du projet.
    pub forge_path: &'a str,
    /// Interfaces générées ; l'image sert l'application web React si elle
    /// existe, sinon la version web de l'application Flutter.
    pub frontends: &'a [Frontend],
}

impl Infra<'_> {
    pub(crate) fn files(&self, renderer: &Renderer) -> Result<Vec<OutputFile>, Error> {
        let context = serde_json::json!({
            "app_name": self.app_name,
            "crate_name": self.app_name,
            "forge_path": self.forge_path,
            "flutter": self.frontends.contains(&Frontend::Flutter),
            "web": self.frontends.contains(&Frontend::Web),
        });
        [
            ("Dockerfile", "infra/dockerfile"),
            (".dockerignore", "infra/dockerignore"),
            ("docker-compose.yml", "infra/compose.yml"),
            (".env.example", "infra/env_example"),
            (".github/workflows/ci.yml", "infra/ci.yml"),
            ("scripts/use-forge.sh", "infra/use_forge.sh"),
        ]
        .into_iter()
        .map(|(path, template)| {
            Ok(OutputFile {
                path: PathBuf::from(path),
                content: renderer.render(template, &context)?,
                policy: Policy::Once,
            })
        })
        .collect()
    }
}
