use std::fmt;

/// Problème localisé dans le fichier d'entrée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Chemin JSON, par exemple `tables[2].columns[4].formula`.
    pub path: String,
    pub message: String,
}

impl Issue {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

/// Ensemble des problèmes trouvés : la validation ne s'arrête pas au premier.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct SchemaError {
    pub issues: Vec<Issue>,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.issues.len();
        writeln!(f, "{count} erreur(s) dans le schéma :")?;
        for issue in &self.issues {
            writeln!(f, "  - {issue}")?;
        }
        Ok(())
    }
}
