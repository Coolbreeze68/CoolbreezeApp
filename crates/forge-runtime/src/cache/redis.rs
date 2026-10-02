//! Cache Redis (feature `redis`), partagé par toutes les instances du serveur.

use std::time::Duration;

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use super::Cache;
use crate::error::Error;

/// Cache Redis : entrées sous `forge:<espace>:` avec une durée de vie, versions
/// des tables sous `forge:<espace>:version:<table>` (sans expiration). Une erreur
/// Redis est journalisée et la lecture se fait en base.
#[derive(Clone)]
pub struct RedisCache {
    connection: ConnectionManager,
    prefix: String,
    ttl: Duration,
}

impl std::fmt::Debug for RedisCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisCache")
            .field("prefix", &self.prefix)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl RedisCache {
    /// Se connecte à `url` (`redis://hôte:6379/0`) ; `namespace` sépare les
    /// applications qui partagent un serveur Redis (le nom de l'application).
    pub async fn connect(url: &str, namespace: &str, ttl: Duration) -> Result<Self, Error> {
        let config_error = |err: redis::RedisError| Error::Config(format!("Redis : {err}"));
        let client = redis::Client::open(url).map_err(config_error)?;
        let connection = ConnectionManager::new(client).await.map_err(config_error)?;
        Ok(Self {
            connection,
            prefix: format!("forge:{namespace}:"),
            ttl,
        })
    }

    fn version_key(&self, table: &str) -> String {
        format!("{}version:{table}", self.prefix)
    }
}

fn warn(err: &redis::RedisError) {
    tracing::warn!(error = %err, "cache Redis indisponible");
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut connection = self.connection.clone();
        connection
            .get(format!("{}{key}", self.prefix))
            .await
            .inspect_err(warn)
            .ok()
            .flatten()
    }

    async fn set(&self, key: &str, value: String) {
        let mut connection = self.connection.clone();
        let result: Result<(), _> = connection
            .set_ex(
                format!("{}{key}", self.prefix),
                value,
                self.ttl.as_secs().max(1),
            )
            .await;
        result.inspect_err(warn).ok();
    }

    async fn versions(&self, tables: &[String]) -> Option<Vec<u64>> {
        if tables.is_empty() {
            return Some(Vec::new());
        }
        let keys: Vec<String> = tables.iter().map(|t| self.version_key(t)).collect();
        let mut connection = self.connection.clone();
        let versions: Result<Vec<Option<u64>>, _> = connection.mget(keys).await;
        let versions = versions.inspect_err(warn).ok()?;
        Some(
            versions
                .into_iter()
                .map(Option::unwrap_or_default)
                .collect(),
        )
    }

    async fn bump(&self, tables: &[String]) {
        let mut pipeline = redis::pipe();
        for table in tables {
            pipeline.incr(self.version_key(table), 1).ignore();
        }
        let mut connection = self.connection.clone();
        let result: Result<(), _> = pipeline.query_async(&mut connection).await;
        if let Err(err) = result {
            tracing::error!(error = %err, ?tables, "cache Redis : invalidation impossible");
        }
    }
}
