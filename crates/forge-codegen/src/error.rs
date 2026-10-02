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
        "le stockage du schéma a changé depuis la dernière migration ({changes}). \
         La génération de migrations incrémentales arrive en phase 2 : en attendant, \
         restaurez la structure précédente ou recréez la base et supprimez \
         `.forge/snapshot.json` et `backend/src/migrations/`."
    )]
    StorageChanged { changes: String },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
