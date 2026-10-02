//! Cache en mémoire (moka), propre à chaque instance du serveur.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::Cache;

/// Cache en mémoire : entrées expirées après `ttl`, `capacity` entrées au plus
/// (les moins utilisées sont évincées). Avec plusieurs instances du serveur,
/// chacune ne voit que ses propres écritures : utilisez alors Redis.
#[derive(Debug)]
pub struct MemoryCache {
    entries: moka::future::Cache<String, String>,
    versions: Mutex<HashMap<String, u64>>,
}

impl MemoryCache {
    pub fn new(ttl: Duration, capacity: u64) -> Self {
        Self {
            entries: moka::future::Cache::builder()
                .time_to_live(ttl)
                .max_capacity(capacity)
                .build(),
            versions: Mutex::default(),
        }
    }

    fn counters(&self) -> std::sync::MutexGuard<'_, HashMap<String, u64>> {
        // Un verrou empoisonné ne contient que des compteurs : on le réutilise.
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        self.entries.get(key).await
    }

    async fn set(&self, key: &str, value: String) {
        self.entries.insert(key.to_owned(), value).await;
    }

    async fn versions(&self, tables: &[String]) -> Option<Vec<u64>> {
        let versions = self.counters();
        Some(
            tables
                .iter()
                .map(|t| versions.get(t).copied().unwrap_or(0))
                .collect(),
        )
    }

    async fn bump(&self, tables: &[String]) {
        let mut versions = self.counters();
        for table in tables {
            *versions.entry(table.clone()).or_default() += 1;
        }
    }
}
