//! Import et export CSV d'une table.
//!
//! ```text
//! GET  /api/opportunite/export?etape=gagne&sort=-montant&delimiter=;
//! POST /api/opportunite/import          (corps : le fichier CSV)
//! ```
//!
//! Export : `id`, les colonnes du schéma (calculées comprises), `owner`,
//! `created_at`, `updated_at` ; filtres, recherche et tri de liste appliqués,
//! ainsi que le périmètre de lecture. Une cellule vide vaut `null` ; une
//! `reference_list` s'écrit `1,2,3`.
//!
//! Import : la première ligne nomme les colonnes (séparateur `,` ou `;`, détecté).
//! Une ligne avec un `id` modifie l'enregistrement, sans `id` elle le crée (une
//! cellule vide prend alors la valeur par défaut). Les colonnes en lecture seule
//! (calculées, système) sont ignorées : un export peut être réimporté. Tout ou
//! rien : à la moindre erreur, rien n'est enregistré et la réponse `422` liste
//! les erreurs de chaque ligne (`error.lines`).

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use forge_schema::spec::{Column, ColumnType, Table};
use forge_schema::value::{self, TypedValue};
use serde_json::{Map, Value as JsonValue, json};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::columns;
use crate::error::{Error, FieldErrors, RowError};
use crate::query::{ListQuery, pairs};
use crate::resource::{ImportRow, Service};

/// Colonnes système placées après les colonnes du schéma.
const TRAILING: [&str; 3] = ["owner", "created_at", "updated_at"];

/// Colonnes exportées, dans l'ordre.
fn headers(table: &Table) -> Vec<&str> {
    std::iter::once("id")
        .chain(table.columns.iter().map(|c| c.name.as_str()))
        .chain(TRAILING)
        .collect()
}

pub(crate) async fn export(
    service: Arc<dyn Service>,
    State(state): State<AppState>,
    user: CurrentUser,
    RawQuery(raw): RawQuery,
) -> Result<Response, Error> {
    let table = state.table(service.name());
    let mut delimiter = b',';
    let mut rest = Vec::new();
    for (key, value) in pairs(raw.as_deref())? {
        match (key.as_str(), value.as_str()) {
            ("delimiter", ",") => delimiter = b',',
            ("delimiter", ";") => delimiter = b';',
            ("delimiter", _) => {
                return Err(Error::validation("delimiter", "`,` ou `;` attendu"));
            }
            _ => rest.push((key, value)),
        }
    }
    let query = ListQuery::from_pairs(table, rest)?;
    let records = service.all(&state, &user, &query).await?;
    let body = write(table, &records, delimiter)?;
    let disposition = format!("attachment; filename=\"{}.csv\"", table.name);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        body,
    )
        .into_response())
}

pub(crate) async fn import(
    service: Arc<dyn Service>,
    State(state): State<AppState>,
    user: CurrentUser,
    body: String,
) -> Result<Json<JsonValue>, Error> {
    let rows = read(state.table(service.name()), &body)?;
    let report = service.import(&state, &user, rows).await?;
    Ok(Json(json!(report)))
}

/// Écrit des enregistrements JSON en CSV.
fn write(table: &Table, records: &[JsonValue], delimiter: u8) -> Result<Vec<u8>, Error> {
    let headers = headers(table);
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    let csv_error = |err: csv::Error| Error::Config(format!("écriture CSV : {err}"));
    writer.write_record(&headers).map_err(csv_error)?;
    for record in records {
        writer
            .write_record(headers.iter().map(|h| cell(&record[*h])))
            .map_err(csv_error)?;
    }
    writer
        .into_inner()
        .map_err(|err| Error::Config(format!("écriture CSV : {err}")))
}

/// Valeur JSON d'une réponse → texte de cellule.
fn cell(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => String::new(),
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(items) => items.iter().map(cell).collect::<Vec<_>>().join(","),
        // Fichier : son identifiant, réimportable.
        JsonValue::Object(file) => file.get("id").map_or_else(String::new, cell),
        other => other.to_string(),
    }
}

/// Rôle d'une colonne du fichier importé.
enum Field<'a> {
    Id,
    Column(&'a Column),
    /// Colonne connue en lecture seule : ignorée.
    Ignored,
}

/// Lit un fichier CSV en lignes d'import. Une erreur d'en-tête refuse tout le
/// fichier ; les erreurs de cellule sont rattachées à leur ligne.
fn read(table: &Table, text: &str) -> Result<Vec<ImportRow>, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(detect_delimiter(text))
        .trim(csv::Trim::Headers)
        .from_reader(text.as_bytes());
    let header_error =
        |errors: FieldErrors| Error::Import(vec![RowError::new(1, &Error::Validation(errors))]);
    let headers = reader
        .headers()
        .map_err(|err| Error::BadRequest(format!("en-tête CSV illisible : {err}")))?
        .clone();
    let fields = classify(table, &headers).map_err(header_error)?;

    let mut rows = Vec::new();
    for result in reader.records() {
        let row = match result {
            Ok(record) => {
                let line = record.position().map_or(0, csv::Position::line);
                parse_row(&fields, &headers, &record, line)
            }
            Err(err) => ImportRow {
                line: err.position().map_or(0, csv::Position::line),
                id: None,
                body: Err(Error::BadRequest(format!("ligne CSV illisible : {err}"))),
            },
        };
        rows.push(row);
    }
    Ok(rows)
}

/// `;` s'il est plus fréquent que `,` dans l'en-tête (fichiers Excel en français).
fn detect_delimiter(text: &str) -> u8 {
    let first = text.lines().next().unwrap_or_default();
    if first.matches(';').count() > first.matches(',').count() {
        b';'
    } else {
        b','
    }
}

fn classify<'a>(
    table: &'a Table,
    headers: &csv::StringRecord,
) -> Result<Vec<Field<'a>>, FieldErrors> {
    let mut errors = FieldErrors::new();
    let mut seen = BTreeSet::new();
    let mut fields = Vec::new();
    for name in headers {
        if !seen.insert(name) {
            errors
                .entry(name.to_owned())
                .or_default()
                .push("colonne en double".into());
        }
        let field = match columns::find(table, name) {
            Some(column) if columns::is_writable(column) => Field::Column(column),
            Some(_) => Field::Ignored,
            None if name == "id" => Field::Id,
            None if columns::system_type(name).is_some() => Field::Ignored,
            None => {
                errors
                    .entry(name.to_owned())
                    .or_default()
                    .push("colonne inconnue".into());
                Field::Ignored
            }
        };
        fields.push(field);
    }
    if errors.is_empty() {
        Ok(fields)
    } else {
        Err(errors)
    }
}

fn parse_row(
    fields: &[Field<'_>],
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    line: u64,
) -> ImportRow {
    let mut errors = FieldErrors::new();
    let mut id = None;
    // Une ligne sans identifiant est une création : ses cellules vides sont omises
    // (valeurs par défaut) ; en modification, elles valent `null`.
    let creating = fields
        .iter()
        .zip(record)
        .all(|(field, text)| !matches!(field, Field::Id) || text.is_empty());
    let mut body = Map::new();
    for ((field, name), text) in fields.iter().zip(headers).zip(record) {
        match field {
            Field::Id if !text.is_empty() => match text.parse() {
                Ok(value) => id = Some(value),
                Err(_) => errors
                    .entry(name.to_owned())
                    .or_default()
                    .push("identifiant entier attendu".into()),
            },
            Field::Column(_) if text.is_empty() && creating => {}
            Field::Column(column) => match to_json(column, text) {
                Ok(value) => {
                    body.insert(column.name.clone(), value);
                }
                Err(message) => errors.entry(name.to_owned()).or_default().push(message),
            },
            Field::Id | Field::Ignored => {}
        }
    }
    let body = if errors.is_empty() {
        Ok(JsonValue::Object(body))
    } else {
        Err(Error::Validation(errors))
    };
    ImportRow { line, id, body }
}

/// Cellule → valeur JSON attendue par l'API (validée ensuite comme un corps REST).
fn to_json(column: &Column, text: &str) -> Result<JsonValue, String> {
    if text.is_empty() {
        return Ok(JsonValue::Null);
    }
    if column.ty == ColumnType::ReferenceList {
        return text
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<i64>().map(JsonValue::from))
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array)
            .map_err(|_| "identifiants entiers séparés par des virgules attendus".to_owned());
    }
    Ok(
        match value::from_text(column.ty, value::Domain::of(column), text)? {
            TypedValue::Null => JsonValue::Null,
            TypedValue::String(s) => json!(s),
            TypedValue::Integer(n) => json!(n),
            TypedValue::Decimal(d) => json!(d.to_string()),
            TypedValue::Boolean(b) => json!(b),
            TypedValue::Date(d) => json!(d.to_string()),
            TypedValue::Datetime(d) => json!(d.to_rfc3339()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        serde_json::from_value(json!({
            "name": "opportunite",
            "columns": [
                { "name": "titre", "type": "string", "required": true },
                { "name": "montant", "type": "decimal" },
                { "name": "ttc", "type": "decimal", "formula": "montant * 1.2" },
                { "name": "probabilite", "type": "integer", "default": 50 },
                { "name": "tags", "type": "reference_list", "target": "tag" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn export_writes_every_column() {
        let records = [json!({
            "id": 1, "titre": "Contrat, phase 1", "montant": "12.5", "ttc": "15",
            "probabilite": null, "tags": [2, 3], "owner": 1,
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"
        })];
        let csv = String::from_utf8(write(&table(), &records, b',').unwrap()).unwrap();
        let mut lines = csv.lines();
        assert_eq!(
            lines.next(),
            Some("id,titre,montant,ttc,probabilite,tags,owner,created_at,updated_at")
        );
        assert_eq!(
            lines.next(),
            Some(
                "1,\"Contrat, phase 1\",12.5,15,,\"2,3\",1,2026-01-01T00:00:00Z,2026-01-01T00:00:00Z"
            )
        );
    }

    #[test]
    fn import_reads_rows() {
        let text = "\u{feff}id;titre;montant;ttc;probabilite;tags;owner\n\
                    ;Nouveau;10;;;1,2;\n\
                    7;Existant;;;;;\n\
                    x;;abc;;;a;\n";
        let rows = read(&table(), text).unwrap();
        assert_eq!(rows.len(), 3);

        assert_eq!((rows[0].line, rows[0].id), (2, None));
        let created = rows[0].body.as_ref().unwrap();
        assert_eq!(
            created,
            &json!({ "titre": "Nouveau", "montant": "10", "tags": [1, 2] })
        );

        assert_eq!(rows[1].id, Some(7));
        let updated = rows[1].body.as_ref().unwrap();
        assert_eq!(
            updated["montant"],
            JsonValue::Null,
            "vide = null en modification"
        );

        let Err(Error::Validation(errors)) = &rows[2].body else {
            panic!("erreurs de cellule attendues");
        };
        assert_eq!(errors.keys().collect::<Vec<_>>(), ["id", "montant", "tags"]);
    }

    #[test]
    fn unknown_or_duplicate_headers_reject_the_file() {
        let Err(Error::Import(lines)) = read(&table(), "titre,inconnu,titre\nx,y,z\n") else {
            panic!("en-tête refusé attendu");
        };
        assert_eq!(lines[0].line, 1);
        let fields = &lines[0].error["fields"];
        assert!(
            fields["inconnu"].is_array() && fields["titre"].is_array(),
            "{fields}"
        );
    }
}
