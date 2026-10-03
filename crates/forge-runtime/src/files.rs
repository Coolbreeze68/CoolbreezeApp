//! Fichiers téléversés : colonnes `file` et `image`.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `POST` | `/api/files?table=…&column=…&name=…` | téléverse le corps de la requête → description du fichier |
//! | `GET` | `/api/files/{id}?expires=…&signature=…` | contenu, par URL signée |
//!
//! Un fichier téléversé n'appartient d'abord qu'à son auteur ; écrire son `id`
//! dans la colonne (création ou modification) le rattache à l'enregistrement.
//! Remplacé, vidé ou supprimé avec l'enregistrement, il est retiré du stockage
//! une fois la transaction validée. Un fichier jamais rattaché est supprimé
//! au bout d'un jour.
//!
//! La lecture d'un enregistrement remplace l'identifiant par sa description
//! (`id`, `name`, `size`, `content_type`, `url`) : l'URL, signée et temporaire,
//! hérite ainsi des règles de lecture de la table, et s'utilise telle quelle
//! dans une balise `<img>`.

use std::collections::BTreeMap;
use std::fmt::{Debug, Write};
use std::io;
use std::path::PathBuf;

use async_trait::async_trait;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use forge_schema::Model;
use forge_schema::spec::{Action, Column, ColumnType, Table};
use forge_schema::value::TypedValue;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use crate::app::AppState;
use crate::auth::{CurrentUser, random_bytes};
use crate::error::Error;
use crate::hooks::Written;
use crate::rules;

/// Préfixe des routes de fichiers ; leur lecture (`GET`) se passe de jeton.
pub(crate) const FILES_PREFIX: &str = "/api/files";

/// Durée de vie d'un fichier jamais rattaché à un enregistrement.
const ORPHAN_TTL: chrono::TimeDelta = chrono::TimeDelta::days(1);

/// Fenêtre de validité des URL signées, en secondes. Les URL produites dans une
/// même fenêtre sont identiques (cache du navigateur) et expirent à la fin de
/// la fenêtre suivante ; le cache des lectures change de clé à chaque fenêtre.
const URL_WINDOW: i64 = 3600;

/// Numéro de la fenêtre de validité en cours.
pub(crate) fn url_window() -> i64 {
    chrono::Utc::now().timestamp() / URL_WINDOW
}

/// Stockage du contenu des fichiers, par identifiant.
#[async_trait]
pub trait Storage: Send + Sync + Debug {
    async fn put(&self, id: &str, content: &[u8]) -> io::Result<()>;
    async fn get(&self, id: &str) -> io::Result<Vec<u8>>;
    /// Supprime le contenu ; un fichier déjà absent n'est pas une erreur.
    async fn delete(&self, id: &str) -> io::Result<()>;
}

/// Stockage sur disque, dans `root` (créé au premier fichier).
#[derive(Debug, Clone)]
pub struct DiskStorage {
    root: PathBuf,
}

impl DiskStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Répartition en sous-dossiers (deux premiers caractères de l'identifiant).
    /// L'identifiant est un UUID validé : aucun chemin ne sort de `root`.
    fn path(&self, id: &str) -> PathBuf {
        self.root.join(&id[..2]).join(id)
    }
}

#[async_trait]
impl Storage for DiskStorage {
    async fn put(&self, id: &str, content: &[u8]) -> io::Result<()> {
        let path = self.path(id);
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(path, content).await
    }

    async fn get(&self, id: &str) -> io::Result<Vec<u8>> {
        tokio::fs::read(self.path(id)).await
    }

    async fn delete(&self, id: &str) -> io::Result<()> {
        match tokio::fs::remove_file(self.path(id)).await {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

/// Métadonnées des fichiers (table système `forge_files`).
pub(crate) mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "forge_files")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub name: String,
        pub content_type: String,
        pub size: i64,
        pub uploaded_by: Option<i64>,
        pub table_name: Option<String>,
        pub column_name: Option<String>,
        pub record_id: Option<i64>,
        pub created_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use entity::{Column as F, Entity as Files};

pub(crate) fn router(model: &Model) -> Router<AppState> {
    let limit = model
        .tables()
        .iter()
        .flat_map(|t| &t.columns)
        .filter(|c| c.ty.is_file())
        .map(Column::max_size_bytes)
        .max()
        .unwrap_or(0);
    Router::new()
        .route(
            FILES_PREFIX,
            post(upload).layer(DefaultBodyLimit::max(
                usize::try_from(limit).unwrap_or(usize::MAX),
            )),
        )
        .route(&format!("{FILES_PREFIX}/{{id}}"), get(download))
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    table: String,
    column: String,
    name: String,
}

async fn upload(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    content: Bytes,
) -> Result<Response, Error> {
    let table = state
        .model
        .table(&query.table)
        .ok_or_else(|| Error::BadRequest(format!("table `{}` inconnue", query.table)))?;
    let column = table
        .columns
        .iter()
        .find(|c| c.name == query.column && c.ty.is_file())
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "`{}` n'est pas une colonne fichier de `{}`",
                query.column, table.name
            ))
        })?;
    // Téléverser sert à créer ou modifier : l'un des deux droits est exigé.
    match rules::scope(&state, table, Action::Create, &user).await {
        Err(Error::Forbidden(_)) => {
            rules::scope(&state, table, Action::Update, &user).await?;
        }
        result => {
            result?;
        }
    }
    let name = file_name(&query.name);
    let content_type = check_content(column, &name, &headers, &content)
        .map_err(|message| Error::validation(&column.name, message))?;

    let id = new_id();
    state.storage.put(&id, &content).await?;
    let record = entity::ActiveModel {
        id: Set(id.clone()),
        name: Set(name),
        content_type: Set(content_type),
        size: Set(i64::try_from(content.len()).unwrap_or(i64::MAX)),
        uploaded_by: Set(Some(user.id)),
        table_name: Set(Some(table.name.clone())),
        column_name: Set(Some(column.name.clone())),
        record_id: Set(None),
        created_at: Set(chrono::Utc::now()),
    };
    let saved = match record.insert(&state.db).await {
        Ok(saved) => saved,
        Err(err) => {
            // Contenu sans métadonnées : retiré, l'erreur d'origine prime.
            if let Err(err) = state.storage.delete(&id).await {
                tracing::warn!("fichier {id} non supprimé : {err}");
            }
            return Err(err.into());
        }
    };
    remove_orphans(&state).await;
    let body = describe(&state, &saved);
    Ok((axum::http::StatusCode::CREATED, axum::Json(body)).into_response())
}

#[derive(Debug, Deserialize)]
struct Signed {
    expires: i64,
    signature: String,
}

async fn download(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(signed): Query<Signed>,
) -> Result<Response, Error> {
    let message = format!("{id}:{}", signed.expires);
    let valid = signed.expires >= chrono::Utc::now().timestamp()
        && state.auth.verify_signature(&message, &signed.signature);
    if !valid {
        return Err(Error::Forbidden(
            "lien de fichier invalide ou expiré".into(),
        ));
    }
    let file = Files::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or(Error::NotFound)?;
    let content = match state.storage.get(&id).await {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(Error::NotFound),
        result => result?,
    };
    let disposition = if is_image(&file.content_type) {
        "inline"
    } else {
        "attachment"
    };
    let headers = [
        (header::CONTENT_TYPE, header_value(&file.content_type)),
        (
            header::CONTENT_DISPOSITION,
            header_value(&format!(
                "{disposition}; filename*=UTF-8''{}",
                percent_encode(&file.name)
            )),
        ),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=3600"),
        ),
    ];
    Ok((headers, content).into_response())
}

/// Rattache les fichiers écrits dans les colonnes de `record_id` ; les fichiers
/// qu'ils remplacent sont détachés (supprimés après validation).
pub(crate) async fn attach(
    db: &impl ConnectionTrait,
    user: &CurrentUser,
    table: &Table,
    record_id: i64,
    values: &[(String, ColumnType, TypedValue)],
    written: &Written,
) -> Result<(), Error> {
    for (name, ty, value) in values {
        if !ty.is_file() {
            continue;
        }
        let new_id = match value {
            TypedValue::String(id) => Some(id.as_str()),
            _ => None,
        };
        let current = Files::find()
            .filter(F::TableName.eq(&table.name))
            .filter(F::ColumnName.eq(name))
            .filter(F::RecordId.eq(record_id))
            .all(db)
            .await?;
        if current.iter().any(|f| Some(f.id.as_str()) == new_id) {
            continue; // inchangé
        }
        detach(db, current, written).await?;
        let Some(new_id) = new_id else { continue };
        let usable = Files::find_by_id(new_id).one(db).await?.filter(|f| {
            f.record_id.is_none()
                && f.table_name.as_deref() == Some(&table.name)
                && f.column_name.as_deref() == Some(name)
                && (f.uploaded_by == Some(user.id) || user.is_admin())
        });
        let Some(file) = usable else {
            return Err(Error::validation(
                name,
                "fichier inconnu, déjà utilisé ou téléversé pour une autre colonne",
            ));
        };
        let mut file: entity::ActiveModel = file.into();
        file.record_id = Set(Some(record_id));
        file.update(db).await?;
    }
    Ok(())
}

/// Détache tous les fichiers d'un enregistrement supprimé.
pub(crate) async fn detach_all(
    db: &impl ConnectionTrait,
    table: &str,
    record_id: i64,
    written: &Written,
) -> Result<(), Error> {
    let files = Files::find()
        .filter(F::TableName.eq(table))
        .filter(F::RecordId.eq(record_id))
        .all(db)
        .await?;
    detach(db, files, written).await
}

async fn detach(
    db: &impl ConnectionTrait,
    files: Vec<entity::Model>,
    written: &Written,
) -> Result<(), Error> {
    if files.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = files.into_iter().map(|f| f.id).collect();
    Files::delete_many()
        .filter(F::Id.is_in(ids.clone()))
        .exec(db)
        .await?;
    for id in ids {
        written.remove_file(id);
    }
    Ok(())
}

/// Retire du stockage des fichiers dont les métadonnées sont supprimées. Un
/// échec est journalisé : le contenu, inaccessible, ne fait qu'occuper de la place.
pub(crate) async fn remove(state: &AppState, ids: &[String]) {
    for id in ids {
        if let Err(err) = state.storage.delete(id).await {
            tracing::warn!("fichier {id} non supprimé du stockage : {err}");
        }
    }
}

/// Supprime les fichiers jamais rattachés, téléversés depuis plus d'un jour.
async fn remove_orphans(state: &AppState) {
    let limit = chrono::Utc::now() - ORPHAN_TTL;
    let orphans = Files::find()
        .filter(F::RecordId.is_null())
        .filter(F::CreatedAt.lt(limit))
        .all(&state.db)
        .await;
    let written = Written::default();
    let result = match orphans {
        Ok(orphans) => detach(&state.db, orphans, &written).await,
        Err(err) => Err(err.into()),
    };
    match result {
        Ok(()) => remove(state, &written.into_parts().1).await,
        Err(err) => tracing::warn!("nettoyage des fichiers orphelins : {err}"),
    }
}

/// Remplace, dans des enregistrements lus, l'identifiant de chaque fichier par
/// sa description (lookups vers une colonne fichier compris).
pub(crate) async fn expand(
    db: &impl ConnectionTrait,
    state: &AppState,
    table: &Table,
    records: &mut [JsonValue],
) -> Result<(), Error> {
    let columns: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| state.model.resolved(&table.name, c).1.ty.is_file())
        .map(|c| c.name.as_str())
        .collect();
    if columns.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = records
        .iter()
        .flat_map(|r| columns.iter().filter_map(|c| r[*c].as_str()))
        .map(str::to_owned)
        .collect();
    let files: BTreeMap<String, entity::Model> = Files::find()
        .filter(F::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|f| (f.id.clone(), f))
        .collect();
    for record in records {
        for column in &columns {
            let described = record[*column]
                .as_str()
                .and_then(|id| files.get(id))
                .map_or(JsonValue::Null, |file| describe(state, file));
            record[*column] = described;
        }
    }
    Ok(())
}

/// Description d'un fichier, avec son URL signée.
fn describe(state: &AppState, file: &entity::Model) -> JsonValue {
    let expires = (url_window() + 2) * URL_WINDOW;
    let signature = state.auth.sign(&format!("{}:{expires}", file.id));
    json!({
        "id": file.id,
        "name": file.name,
        "size": file.size,
        "content_type": file.content_type,
        "url": format!("{FILES_PREFIX}/{}?expires={expires}&signature={signature}", file.id),
    })
}

/// Vérifie le contenu téléversé ; retourne son type MIME.
fn check_content(
    column: &Column,
    name: &str,
    headers: &HeaderMap,
    content: &[u8],
) -> Result<String, String> {
    let max = column.max_size_bytes();
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > max {
        return Err(format!(
            "fichier trop volumineux : {} Mo au maximum",
            max / 1024 / 1024
        ));
    }
    if content.is_empty() {
        return Err("fichier vide".into());
    }
    if column.ty == ColumnType::Image {
        // Type lu dans le contenu, jamais dans l'en-tête : une image déclarée
        // ne peut pas cacher du HTML ou du SVG (scripts).
        return image_type(content)
            .map(str::to_owned)
            .ok_or_else(|| "image PNG, JPEG, GIF ou WebP attendue".into());
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| v.contains('/'))
        .unwrap_or_else(|| "application/octet-stream".into());
    match &column.accept {
        Some(accept) if !accept.iter().any(|p| accepts(p, &content_type, name)) => Err(format!(
            "type de fichier refusé : attendu {}",
            accept.join(", ")
        )),
        _ => Ok(content_type),
    }
}

/// `pattern` (de l'option `accept`) admet-il ce fichier ?
fn accepts(pattern: &str, content_type: &str, name: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    if pattern.starts_with('.') {
        return name.to_ascii_lowercase().ends_with(&pattern);
    }
    match pattern.strip_suffix("/*") {
        Some(kind) => content_type.split_once('/').is_some_and(|(k, _)| k == kind),
        None => content_type == pattern,
    }
}

/// Type d'image reconnu à sa signature.
fn image_type(content: &[u8]) -> Option<&'static str> {
    if content.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if content.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if content.starts_with(b"GIF87a") || content.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if content.len() >= 12 && &content[..4] == b"RIFF" && &content[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn is_image(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    )
}

/// Nom affiché : sans chemin, 255 caractères au plus.
fn file_name(raw: &str) -> String {
    let name = raw.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let name: String = name.chars().filter(|c| !c.is_control()).take(255).collect();
    if name.is_empty() {
        "fichier".into()
    } else {
        name
    }
}

/// Identifiant aléatoire : UUID version 4, en minuscules.
fn new_id() -> String {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&random_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let hex = bytes.iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    });
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Encodage RFC 5987 d'un nom de fichier (`filename*=UTF-8''…`).
fn percent_encode(name: &str) -> String {
    name.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn header_value(value: &str) -> HeaderValue {
    HeaderValue::from_str(value)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_valid_uuids() {
        let id = new_id();
        let typed = forge_schema::value::from_json(
            ColumnType::File,
            forge_schema::value::Domain::default(),
            &json!(id),
        );
        assert!(typed.is_ok(), "{id}");
        assert_ne!(new_id(), id);
    }

    #[test]
    fn accept_patterns() {
        assert!(accepts("application/pdf", "application/pdf", "a.pdf"));
        assert!(accepts("image/*", "image/png", "a.png"));
        assert!(accepts(".CSV", "text/plain", "export.csv"));
        assert!(!accepts("image/*", "text/html", "a.html"));
        assert!(!accepts(".csv", "text/csv", "a.txt"));
    }

    #[test]
    fn images_are_recognized_by_content() {
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"<svg onload=alert(1)>"), None);
    }

    #[test]
    fn names_lose_their_path() {
        assert_eq!(file_name("../../etc/passwd"), "passwd");
        assert_eq!(file_name("C:\\docs\\devis.pdf"), "devis.pdf");
        assert_eq!(file_name("  "), "fichier");
        assert_eq!(percent_encode("devis é.pdf"), "devis%20%C3%A9.pdf");
    }
}
