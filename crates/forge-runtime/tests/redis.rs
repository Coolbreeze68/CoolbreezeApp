//! Cache Redis partagé entre deux instances du serveur. Exige un serveur Redis :
//!
//! ```text
//! TEST_CACHE_URL=redis://localhost:6379 cargo test -p forge-runtime --features redis --test redis -- --ignored
//! ```

use std::time::Duration;

use forge_runtime::cache::{Cache, RedisCache};

async fn connect(namespace: &str, ttl: Duration) -> RedisCache {
    let url = std::env::var("TEST_CACHE_URL").expect("TEST_CACHE_URL (serveur Redis de test)");
    RedisCache::connect(&url, namespace, ttl)
        .await
        .expect("connexion à Redis")
}

/// Espace de noms propre à l'exécution : les tests ne se voient pas entre eux.
fn namespace(test: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("test-{test}-{}-{nanos}", std::process::id())
}

#[tokio::test]
#[ignore = "exige Redis (TEST_CACHE_URL)"]
async fn deux_instances_partagent_entrees_et_versions() {
    let shared = namespace("partage");
    let (a, b) = (
        connect(&shared, Duration::from_secs(60)).await,
        connect(&shared, Duration::from_secs(60)).await,
    );
    let tables = ["client".to_owned(), "commande".to_owned()];
    assert_eq!(a.versions(&tables).await, Some(vec![0, 0]));

    a.set("cle", "valeur".into()).await;
    assert_eq!(b.get("cle").await.as_deref(), Some("valeur"));

    // Une écriture sur une instance invalide les lectures de l'autre.
    b.bump(&tables[1..]).await;
    assert_eq!(a.versions(&tables).await, Some(vec![0, 1]));

    let other = connect(&namespace("autre"), Duration::from_secs(60)).await;
    assert_eq!(other.get("cle").await, None, "espaces de noms séparés");
}

#[tokio::test]
#[ignore = "exige Redis (TEST_CACHE_URL)"]
async fn les_entrees_expirent() {
    let cache = connect(&namespace("expiration"), Duration::from_secs(1)).await;
    cache.set("cle", "valeur".into()).await;
    assert!(cache.get("cle").await.is_some());
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert_eq!(cache.get("cle").await, None);
}

#[tokio::test]
async fn adresse_invalide_refusee_au_demarrage() {
    let err = RedisCache::connect("pas-une-url", "x", Duration::from_secs(1))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Redis"), "{err}");
}
