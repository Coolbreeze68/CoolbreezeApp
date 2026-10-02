//! Écriture des fichiers générés, selon leur politique de régénération.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Réécrit à chaque génération ; supprimé s'il n'est plus produit.
    Generated,
    /// Créé s'il n'existe pas, jamais modifié ensuite (code utilisateur).
    Once,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// Chemin relatif au dossier du projet.
    pub path: PathBuf,
    pub content: String,
    pub policy: Policy,
}

/// Bilan d'une génération (chemins relatifs au projet).
#[derive(Debug, Default)]
pub struct Report {
    pub created: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn is_unchanged(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
}

/// Écrit `files` dans `project`. Un fichier identique n'est pas réécrit (pas de
/// recompilation inutile). Les fichiers des dossiers `generated_dirs` qui ne sont
/// plus produits sont supprimés.
pub(crate) fn write(
    project: &Path,
    files: &[OutputFile],
    generated_dirs: &[&str],
    report: &mut Report,
) -> Result<(), Error> {
    for file in files {
        let target = project.join(&file.path);
        let existing = match fs::read_to_string(&target) {
            Ok(content) => Some(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(Error::io(&target)(err)),
        };
        let unchanged = match (file.policy, &existing) {
            (Policy::Once, Some(_)) => true,
            (Policy::Generated, Some(content)) => *content == file.content,
            (_, None) => false,
        };
        if unchanged {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(Error::io(parent))?;
        }
        fs::write(&target, &file.content).map_err(Error::io(&target))?;
        let list = if existing.is_some() {
            &mut report.updated
        } else {
            &mut report.created
        };
        list.push(file.path.clone());
    }

    let produced: BTreeSet<&Path> = files.iter().map(|f| f.path.as_path()).collect();
    for dir in generated_dirs {
        for path in walk(&project.join(dir))? {
            let relative = path.strip_prefix(project).expect("sous le projet");
            if !produced.contains(relative) {
                fs::remove_file(&path).map_err(Error::io(&path))?;
                report.deleted.push(relative.to_path_buf());
            }
        }
    }
    Ok(())
}

/// Fichiers d'un dossier, récursivement (vide s'il n'existe pas).
fn walk(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(err) => return Err(Error::io(dir)(err)),
    };
    for entry in entries {
        let path = entry.map_err(Error::io(dir))?.path();
        if path.is_dir() {
            files.extend(walk(&path)?);
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, content: &str, policy: Policy) -> OutputFile {
        OutputFile {
            path: path.into(),
            content: content.into(),
            policy,
        }
    }

    #[test]
    fn policies() {
        let dir = tempfile::tempdir().unwrap();
        let files = [
            file("gen/a.rs", "a", Policy::Generated),
            file("custom/b.rs", "b", Policy::Once),
        ];
        let mut report = Report::default();
        write(dir.path(), &files, &["gen"], &mut report).unwrap();
        assert_eq!(report.created.len(), 2);

        // Deuxième passe identique : aucun changement.
        let mut report = Report::default();
        write(dir.path(), &files, &["gen"], &mut report).unwrap();
        assert!(report.is_unchanged());

        // Le code utilisateur n'est jamais écrasé ; le code généré l'est.
        fs::write(dir.path().join("custom/b.rs"), "modifié").unwrap();
        fs::write(dir.path().join("gen/a.rs"), "modifié").unwrap();
        fs::write(dir.path().join("gen/obsolete.rs"), "x").unwrap();
        let mut report = Report::default();
        write(dir.path(), &files, &["gen"], &mut report).unwrap();
        assert_eq!(report.updated, [PathBuf::from("gen/a.rs")]);
        assert_eq!(report.deleted, [PathBuf::from("gen/obsolete.rs")]);
        assert_eq!(
            fs::read_to_string(dir.path().join("custom/b.rs")).unwrap(),
            "modifié"
        );
    }
}
