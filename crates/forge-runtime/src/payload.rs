//! Validation d'un corps JSON de création ou de modification, selon le schéma.

use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::{self, TypedValue};
use serde_json::Value as Json;

use crate::columns;
use crate::error::{Error, FieldErrors};

/// Longueur maximale d'une colonne `string` (`varchar(255)`).
const STRING_MAX_CHARS: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Create,
    Update,
}

/// Corps validé : valeurs des colonnes et liens des `reference_list`.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Payload {
    pub values: Vec<(String, ColumnType, TypedValue)>,
    pub links: Vec<(String, Vec<i64>)>,
}

/// Valide `body` pour `table`. En création, les valeurs par défaut sont ajoutées
/// et les colonnes obligatoires vérifiées.
pub(crate) fn parse(table: &Table, body: Json, mode: Mode) -> Result<Payload, Error> {
    let Json::Object(object) = body else {
        return Err(Error::BadRequest("objet JSON attendu".into()));
    };
    let mut payload = Payload::default();
    let mut errors = FieldErrors::new();

    for (key, json) in object {
        let Some(column) = columns::find(table, &key) else {
            let message = if columns::system_type(&key).is_some() {
                "champ géré automatiquement, en lecture seule"
            } else {
                "champ inconnu"
            };
            push(&mut errors, &key, message.to_owned());
            continue;
        };
        if !columns::is_writable(column) {
            push(
                &mut errors,
                &key,
                "champ calculé, en lecture seule".to_owned(),
            );
            continue;
        }
        if column.ty == ColumnType::ReferenceList {
            match ids(&json) {
                Some(ids) => payload.links.push((key, ids)),
                None => push(
                    &mut errors,
                    &key,
                    "liste d'identifiants entiers attendue".to_owned(),
                ),
            }
            continue;
        }
        match value::from_json(column.ty, column.values.as_deref(), &json) {
            Ok(TypedValue::Null) if column.required => {
                push(&mut errors, &key, "valeur obligatoire".to_owned());
            }
            Ok(TypedValue::String(s))
                if column.ty == ColumnType::String && s.chars().count() > STRING_MAX_CHARS =>
            {
                push(
                    &mut errors,
                    &key,
                    format!("{STRING_MAX_CHARS} caractères au maximum"),
                );
            }
            Ok(typed) => payload.values.push((key, column.ty, typed)),
            Err(message) => push(&mut errors, &key, message),
        }
    }

    if mode == Mode::Create {
        for column in table.columns.iter().filter(|c| columns::is_writable(c)) {
            let present = payload.values.iter().any(|(k, ..)| *k == column.name)
                || payload.links.iter().any(|(k, _)| *k == column.name)
                || errors.contains_key(&column.name);
            if present {
                continue;
            }
            match &column.default {
                Some(default) => {
                    match value::from_json(column.ty, column.values.as_deref(), default) {
                        Ok(typed) => payload.values.push((column.name.clone(), column.ty, typed)),
                        Err(message) => push(&mut errors, &column.name, message),
                    }
                }
                None if column.required => {
                    push(&mut errors, &column.name, "valeur obligatoire".to_owned());
                }
                None => {}
            }
        }
    }

    if errors.is_empty() {
        Ok(payload)
    } else {
        Err(Error::Validation(errors))
    }
}

fn push(errors: &mut FieldErrors, field: &str, message: String) {
    errors.entry(field.to_owned()).or_default().push(message);
}

/// Identifiants d'une `reference_list`, dédoublonnés et triés.
fn ids(json: &Json) -> Option<Vec<i64>> {
    let mut ids = json
        .as_array()?
        .iter()
        .map(Json::as_i64)
        .collect::<Option<Vec<_>>>()?;
    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn table() -> Table {
        serde_json::from_value(json!({
            "name": "opportunite",
            "columns": [
                { "name": "titre", "type": "string", "required": true },
                { "name": "montant", "type": "decimal" },
                { "name": "probabilite", "type": "integer", "default": 50 },
                { "name": "pondere", "type": "decimal", "formula": "montant", "persist": true },
                { "name": "etape", "type": "enum", "values": ["a", "b"] },
                { "name": "tags", "type": "reference_list", "target": "tag" }
            ]
        }))
        .unwrap()
    }

    fn fields(err: Error) -> FieldErrors {
        match err {
            Error::Validation(fields) => fields,
            other => panic!("erreur de validation attendue : {other:?}"),
        }
    }

    #[test]
    fn create_applies_defaults() {
        let payload = parse(
            &table(),
            json!({ "titre": "x", "tags": [3, 1, 3] }),
            Mode::Create,
        )
        .unwrap();
        let names: Vec<_> = payload.values.iter().map(|(k, ..)| k.as_str()).collect();
        assert_eq!(names, ["titre", "probabilite"]);
        assert_eq!(payload.values[1].2, TypedValue::Integer(50));
        assert_eq!(payload.links, vec![("tags".to_owned(), vec![1, 3])]);
    }

    #[test]
    fn create_reports_every_field() {
        let body = json!({
            "id": 1, "pondere": 2, "inconnu": true, "montant": "abc",
            "etape": "z", "tags": ["x"]
        });
        let errors = fields(parse(&table(), body, Mode::Create).unwrap_err());
        let keys: Vec<_> = errors.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "etape", "id", "inconnu", "montant", "pondere", "tags", "titre"
            ]
        );
        assert_eq!(errors["titre"], ["valeur obligatoire"]);
        assert!(errors["id"][0].contains("lecture seule"));
        assert!(errors["pondere"][0].contains("calculé"));
    }

    #[test]
    fn update_is_partial() {
        let payload = parse(&table(), json!({ "montant": 12.5 }), Mode::Update).unwrap();
        assert_eq!(payload.values.len(), 1);

        let errors = fields(parse(&table(), json!({ "titre": null }), Mode::Update).unwrap_err());
        assert_eq!(errors["titre"], ["valeur obligatoire"]);

        let long = "x".repeat(256);
        let errors = fields(parse(&table(), json!({ "titre": long }), Mode::Update).unwrap_err());
        assert!(errors["titre"][0].contains("255"));
    }

    #[test]
    fn body_must_be_an_object() {
        assert!(matches!(
            parse(&table(), json!([1]), Mode::Create),
            Err(Error::BadRequest(_))
        ));
    }
}
