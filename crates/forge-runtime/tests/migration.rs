//! Migrations incrémentales avec données, sur la base `TEST_DATABASE_URL`
//! (SQLite en mémoire par défaut). Attention : la base est vidée.

use forge_runtime::migration::{OnDelete, Plan, TableDef};
use sea_orm::prelude::Decimal;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{
    big_integer, big_integer_null, decimal_len, decimal_len_null, string, string_null,
};

fn client_v1(t: &mut TableDef) {
    t.col(string("nom"));
    t.col(string_null("tel"));
    t.col(string_null("obsolete"));
}

fn commande_v1(t: &mut TableDef) {
    t.col(big_integer("client"));
    t.col(decimal_len_null("montant", 19, 4));
    t.reference("client", "client", OnDelete::Restrict);
}

fn client_v2(t: &mut TableDef) {
    t.col(string("nom"));
    t.col(string_null("telephone"));
    t.col(big_integer("score").default(0));
    t.unique("nom");
}

fn commande_v2(t: &mut TableDef) {
    t.col(big_integer("client"));
    t.col(decimal_len("montant", 19, 4));
    t.col(big_integer_null("parrain"));
    t.reference("client", "client", OnDelete::Restrict);
    t.reference("parrain", "client", OnDelete::SetNull);
}

struct Initial;

impl MigrationName for Initial {
    fn name(&self) -> &'static str {
        "m0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Initial {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .table("client", client_v1)
            .table("commande", commande_v1)
            .apply(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .drop_table("commande")
            .drop_table("client")
            .apply(manager)
            .await
    }
}

struct Changes;

impl MigrationName for Changes {
    fn name(&self) -> &'static str {
        "m0002_changes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Changes {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .rename_column("client", "tel", "telephone")
            .add_column("client", big_integer("score").default(0))
            .drop_column("client", "obsolete")
            .unique("client", "nom")
            .alter_column("commande", decimal_len("montant", 19, 4))
            .add_column("commande", big_integer_null("parrain"))
            .reference("commande", "parrain", "client", OnDelete::SetNull)
            .table("tag", |t| {
                t.col(string("libelle"));
            })
            .join_table("commande_tags", "commande", "tag")
            .redefine("client", client_v2)
            .redefine("commande", commande_v2)
            .apply(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .drop_table("commande_tags")
            .drop_table("tag")
            .drop_reference("commande", "parrain")
            .drop_column("commande", "parrain")
            .alter_column("commande", decimal_len_null("montant", 19, 4))
            .drop_unique("client", "nom")
            .add_column("client", string_null("obsolete"))
            .drop_column("client", "score")
            .rename_column("client", "telephone", "tel")
            .redefine("client", client_v1)
            .redefine("commande", commande_v1)
            .apply(manager)
            .await
    }
}

struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        // `users` est la cible de la colonne système `owner`.
        let mut migrations = forge_runtime::migration::system();
        migrations.push(Box::new(Initial));
        migrations.push(Box::new(Changes));
        migrations
    }
}

async fn connect() -> DatabaseConnection {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let mut options = ConnectOptions::new(url.clone());
    if url.starts_with("sqlite:") {
        options.max_connections(1);
    }
    Database::connect(options).await.expect("connexion")
}

async fn execute(db: &DatabaseConnection, statement: &InsertStatement) -> Result<(), DbErr> {
    db.execute(statement).await.map(drop)
}

fn insert_client(columns: &[&str], values: Vec<SimpleExpr>) -> InsertStatement {
    Query::insert()
        .into_table("client")
        .columns(
            columns
                .iter()
                .chain(&["created_at", "updated_at"])
                .map(|c| Alias::new(*c)),
        )
        .values_panic(
            values
                .into_iter()
                .chain([Expr::current_timestamp(), Expr::current_timestamp()]),
        )
        .to_owned()
}

async fn columns(
    db: &DatabaseConnection,
    table: &str,
    columns: &[&str],
) -> Vec<sea_orm::QueryResult> {
    let select = Query::select()
        .columns(columns.iter().map(|c| Alias::new(*c)))
        .from(Alias::new(table))
        .order_by("id", Order::Asc)
        .to_owned();
    db.query_all(&select).await.expect("lecture")
}

#[tokio::test]
async fn incremental_migration_keeps_data() {
    let db = connect().await;
    Migrator::fresh(&db).await.expect("migrations");
    Migrator::down(&db, Some(1))
        .await
        .expect("retour à la version initiale");

    // Données de la version initiale.
    execute(
        &db,
        &insert_client(
            &["nom", "tel", "obsolete"],
            vec!["Acme".into(), "0102".into(), "x".into()],
        ),
    )
    .await
    .unwrap();
    let commande = Query::insert()
        .into_table("commande")
        .columns(["client", "montant", "created_at", "updated_at"])
        .values_panic([
            1.into(),
            Decimal::new(1250, 1).into(),
            Expr::current_timestamp(),
            Expr::current_timestamp(),
        ])
        .to_owned();
    execute(&db, &commande).await.unwrap();

    Migrator::up(&db, None)
        .await
        .expect("migration incrémentale");

    let rows = columns(&db, "client", &["nom", "telephone", "score"]).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].try_get::<String>("", "nom").unwrap(), "Acme");
    assert_eq!(
        rows[0].try_get::<String>("", "telephone").unwrap(),
        "0102",
        "renommage sans perte"
    );
    assert_eq!(
        rows[0].try_get::<i64>("", "score").unwrap(),
        0,
        "valeur par défaut"
    );
    let rows = columns(&db, "commande", &["client", "montant", "parrain"]).await;
    assert_eq!(
        rows[0].try_get::<Decimal>("", "montant").unwrap(),
        Decimal::new(125, 0)
    );
    assert_eq!(rows[0].try_get::<Option<i64>>("", "parrain").unwrap(), None);

    // Contraintes de la nouvelle version : unicité et nouvelle clé étrangère.
    let duplicate = insert_client(&["nom"], vec!["Acme".into()]);
    assert!(execute(&db, &duplicate).await.is_err(), "nom unique");
    let orphan = Query::update()
        .table("commande")
        .value("parrain", 999)
        .to_owned();
    assert!(
        db.execute(&orphan).await.is_err(),
        "clé étrangère sur parrain"
    );
    assert!(
        db.execute(&Query::delete().from_table("client").to_owned())
            .await
            .is_err(),
        "la clé étrangère existante est conservée"
    );

    // Retour arrière : structure initiale, données conservées.
    Migrator::down(&db, Some(1)).await.expect("annulation");
    let rows = columns(&db, "client", &["nom", "tel", "obsolete"]).await;
    assert_eq!(rows[0].try_get::<String>("", "tel").unwrap(), "0102");
    assert_eq!(
        rows[0].try_get::<Option<String>>("", "obsolete").unwrap(),
        None
    );
    execute(&db, &insert_client(&["nom"], vec!["Acme".into()]))
        .await
        .expect("plus d'unicité");
}
