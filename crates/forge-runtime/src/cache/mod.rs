//! Cache des lectures : listes, lectures et agrégats, quel que soit le point
//! d'entrée (REST, GraphQL).
//!
//! Invalidation par version : chaque table a un numéro de version, incrémenté
//! après chaque écriture validée. La clé d'une lecture contient les versions des
//! tables dont son résultat dépend (la table, les cibles de ses relations, les
//! tables lues par ses lookups et formules calculées à la lecture, les
//! paramètres) : une écriture rend donc inaccessibles toutes les entrées
//! concernées, sans les parcourir. Les entrées périmées expirent avec leur durée
//! de vie. La clé contient aussi le périmètre de l'utilisateur (règles) et, pour
//! une table à formules calculées à la lecture, la date du jour (`TODAY`), et
//! pour une table à fichiers, la fenêtre de validité de leurs URL signées.
//!
//! Les écritures faites hors de forge (hooks, routes personnalisées) doivent
//! être signalées : [`crate::HookContext::modified`], [`crate::AppState::invalidate`].

mod memory;
#[cfg(feature = "redis")]
mod redis;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use forge_schema::Model;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

pub use memory::MemoryCache;
#[cfg(feature = "redis")]
pub use redis::RedisCache;

use crate::error::Error;
use crate::{compute, files};

/// Stockage du cache. Une erreur de stockage ne doit jamais faire échouer une
/// requête : les implémentations la journalisent et se comportent comme un cache vide.
#[async_trait]
pub trait Cache: Send + Sync + fmt::Debug {
    /// Valeur enregistrée sous `key`, si elle existe encore.
    async fn get(&self, key: &str) -> Option<String>;

    /// Enregistre une valeur ; sa durée de vie est fixée par l'implémentation.
    async fn set(&self, key: &str, value: String);

    /// Version actuelle de chaque table (0 si elle n'a jamais été modifiée) ;
    /// `None` si elles sont indisponibles : la lecture contourne alors le cache.
    async fn versions(&self, tables: &[String]) -> Option<Vec<u64>>;

    /// Incrémente la version des tables.
    async fn bump(&self, tables: &[String]);
}

/// Tables dont dépend la lecture d'une table, et dépendance à la date du jour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dependencies {
    pub tables: Vec<String>,
    pub daily: bool,
}

/// Cache des lectures d'une application : stockage et dépendances de chaque table.
#[derive(Debug)]
pub(crate) struct Reads {
    store: Arc<dyn Cache>,
    dependencies: BTreeMap<String, Dependencies>,
    /// Tables dont les lectures portent des URL de fichiers signées.
    with_files: BTreeSet<String>,
}

impl Reads {
    pub(crate) fn new(store: Arc<dyn Cache>, model: &Model) -> Self {
        let dependencies = model
            .tables()
            .iter()
            .map(|t| (t.name.clone(), compute::read_dependencies(model, &t.name)))
            .collect();
        let with_files = model
            .tables()
            .iter()
            .filter(|t| {
                t.columns
                    .iter()
                    .any(|c| model.resolved(&t.name, c).1.ty.is_file())
            })
            .map(|t| t.name.clone())
            .collect();
        Self {
            store,
            dependencies,
            with_files,
        }
    }

    /// Résultat d'une lecture de `table`, depuis le cache ou calculé par `load`.
    /// `request` décrit la lecture complètement (opération, paramètres, périmètre).
    pub(crate) async fn get_or_load<T, F, Fut>(
        &self,
        table: &str,
        request: &str,
        load: F,
    ) -> Result<T, Error>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let Some(key) = self.key(table, request).await else {
            return load().await;
        };
        let hit = self
            .store
            .get(&key)
            .await
            .and_then(|text| serde_json::from_str(&text).ok());
        let result = if hit.is_some() { "hit" } else { "miss" };
        metrics::counter!("forge_cache_requests_total", "table" => table.to_owned(), "result" => result)
            .increment(1);
        if let Some(value) = hit {
            return Ok(value);
        }
        let value = load().await?;
        if let Ok(text) = serde_json::to_string(&value) {
            self.store.set(&key, text).await;
        }
        Ok(value)
    }

    /// Clé : table, empreinte de la lecture et des versions de ses dépendances.
    async fn key(&self, table: &str, request: &str) -> Option<String> {
        let dependencies = self.dependencies.get(table);
        let tables = dependencies
            .map(|d| d.tables.as_slice())
            .unwrap_or_default();
        let versions = self.store.versions(tables).await?;
        let mut hasher = Sha256::new();
        hasher.update(request.as_bytes());
        for (table, version) in tables.iter().zip(versions) {
            hasher.update(format!("|{table}={version}").as_bytes());
        }
        if dependencies.is_some_and(|d| d.daily) {
            hasher.update(chrono::Utc::now().date_naive().to_string().as_bytes());
        }
        // Une lecture en cache ne sert que dans sa fenêtre : ses URL restent valides.
        if self.with_files.contains(table) {
            hasher.update(files::url_window().to_string().as_bytes());
        }
        Some(format!(
            "{table}:{}",
            URL_SAFE_NO_PAD.encode(hasher.finalize())
        ))
    }

    /// Invalide les lectures qui dépendent de `tables`.
    pub(crate) async fn invalidate(&self, tables: BTreeSet<String>) {
        if !tables.is_empty() {
            self.store
                .bump(&tables.into_iter().collect::<Vec<_>>())
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    fn model() -> Model {
        Model::from_json(
            &json!({
                "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
                "roles": ["admin"],
                "parameters": [{ "name": "tva", "type": "decimal", "default": 20 }],
                "tables": [
                    { "name": "client", "columns": [
                        { "name": "nom", "type": "string" },
                        { "name": "nb_commandes", "type": "integer", "formula": "COUNT(commandes)" }
                    ] },
                    { "name": "commande", "columns": [
                        { "name": "client", "type": "reference", "target": "client", "inverse": "commandes" },
                        { "name": "montant", "type": "decimal" },
                        { "name": "ttc", "type": "decimal", "formula": "montant * (1 + $param.tva / 100)", "persist": true },
                        { "name": "nom_client", "type": "lookup", "path": "client.nom" }
                    ] },
                    { "name": "note", "columns": [{ "name": "texte", "type": "text" }] }
                ]
            })
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn dependencies_follow_relations_and_computed_columns() {
        let reads = Reads::new(
            Arc::new(MemoryCache::new(Duration::from_secs(60), 100)),
            &model(),
        );
        let deps = |table: &str| reads.dependencies[table].clone();
        assert_eq!(
            deps("client"),
            Dependencies {
                tables: vec!["client".into(), "commande".into(), "users".into()],
                daily: true
            },
            "agrégat calculé à la lecture"
        );
        // Formule persistée : stockée dans la table, recalculée à l'écriture.
        assert_eq!(
            deps("commande"),
            Dependencies {
                tables: vec!["client".into(), "commande".into(), "users".into()],
                daily: false
            }
        );
        assert_eq!(deps("note").tables, ["note", "users"]);
    }

    #[tokio::test]
    async fn writes_change_the_keys_of_dependent_reads() {
        let reads = Reads::new(
            Arc::new(MemoryCache::new(Duration::from_secs(60), 100)),
            &model(),
        );
        let load = |n: i64| async move { Ok::<_, Error>(n) };
        assert_eq!(
            reads
                .get_or_load("commande", "liste", || load(1))
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            reads
                .get_or_load("commande", "liste", || load(2))
                .await
                .unwrap(),
            1,
            "en cache"
        );
        assert_eq!(
            reads
                .get_or_load("commande", "autre", || load(3))
                .await
                .unwrap(),
            3
        );

        reads.invalidate(BTreeSet::from(["note".to_owned()])).await;
        assert_eq!(
            reads
                .get_or_load("commande", "liste", || load(4))
                .await
                .unwrap(),
            1,
            "sans rapport"
        );
        reads
            .invalidate(BTreeSet::from(["client".to_owned()]))
            .await;
        assert_eq!(
            reads
                .get_or_load("commande", "liste", || load(5))
                .await
                .unwrap(),
            5,
            "dépendance modifiée"
        );
    }
}
