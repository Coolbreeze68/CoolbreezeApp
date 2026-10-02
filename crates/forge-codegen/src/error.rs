use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("template {0}")]
    Template(String),
    #[error("{path} : {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} : {message}")]
    Snapshot { path: PathBuf, message: String },
    #[error(
        "la migration supprimerait ou convertirait des données :\n{}\n\
         Relancez avec `--allow-destructive` pour la générer quand même, ou utilisez \
         `renamed_from` si une colonne a seulement été renommée.",
        changes.iter().map(|c| format!("  - {c}")).collect::<Vec<_>>().join("\n")
    )]
    Destructive { changes: Vec<String> },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
