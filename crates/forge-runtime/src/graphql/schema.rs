//! Construction du schéma GraphQL à partir du modèle (noms : [`forge_schema::graphql`]).
//!
//! Chaque résolveur délègue au [`Service`] de la table : règles, validation,
//! hooks et calculs sont ceux de l'API REST.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_graphql::dataloader::DataLoader;
use async_graphql::dynamic::{
    Enum, Field, FieldFuture, FieldValue, InputObject, InputValue, Object, ObjectAccessor,
    ResolverContext, Scalar, Schema, SchemaBuilder, TypeRef, ValueAccessor,
};
use async_graphql::{Name, Value};
use forge_schema::graphql as names;
use forge_schema::spec::{Column, ColumnType, Label, Table};
use forge_schema::{ColumnRef, Model};
use serde_json::Value as JsonValue;

use super::{Extensions, RecordLoader, to_graphql};
use crate::aggregate::AggregateQuery;
use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::compute;
use crate::error::Error;
use crate::parameters;
use crate::query::ListQuery;
use crate::resource::{Listing, Service};

type Services = BTreeMap<String, Arc<dyn Service>>;
type GraphQlResult<T> = async_graphql::Result<T>;

/// Nature GraphQL d'une valeur, selon le type de colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Id,
    Int,
    Decimal,
    String,
    Boolean,
    Date,
    DateTime,
}

const KINDS: [Kind; 7] = [
    Kind::Id,
    Kind::Int,
    Kind::Decimal,
    Kind::String,
    Kind::Boolean,
    Kind::Date,
    Kind::DateTime,
];

impl Kind {
    fn of(ty: ColumnType) -> Self {
        match ty {
            ColumnType::Reference => Self::Id,
            ColumnType::Integer | ColumnType::Duration => Self::Int,
            ColumnType::Decimal => Self::Decimal,
            ColumnType::Boolean => Self::Boolean,
            ColumnType::Date => Self::Date,
            ColumnType::Datetime => Self::DateTime,
            ColumnType::String
            | ColumnType::Text
            | ColumnType::Enum
            | ColumnType::ReferenceList
            | ColumnType::Lookup => Self::String,
            model => Self::of(model.base()),
        }
    }

    fn scalar(self) -> &'static str {
        match self {
            Self::Id => TypeRef::ID,
            Self::Int => TypeRef::INT,
            Self::Decimal => "Decimal",
            Self::String => TypeRef::STRING,
            Self::Boolean => TypeRef::BOOLEAN,
            Self::Date => "Date",
            Self::DateTime => "DateTime",
        }
    }

    fn filter(self) -> String {
        format!("{}Filter", self.scalar())
    }

    /// Valeur JSON d'un enregistrement → valeur GraphQL (`None` si nulle).
    fn output(self, json: &JsonValue) -> Option<Value> {
        match (self, json) {
            (_, JsonValue::Null) => None,
            // Le type `ID` se sérialise en texte.
            (Self::Id, JsonValue::Number(n)) => Some(Value::String(n.to_string())),
            _ => Value::from_json(json.clone()).ok(),
        }
    }
}

/// Schéma complet : types et opérations de chaque table, paramètres, extensions.
pub(super) fn build(
    model: &Model,
    services: &Services,
    extensions: Extensions,
) -> Result<Schema, Error> {
    let mut query = Object::new("Query");
    let mut mutation = Object::new("Mutation");
    let mut builder = builtin_types(Schema::build("Query", Some("Mutation"), None));
    let locale = &model.spec().app.default_locale;

    for table in model.tables() {
        let service = services
            .get(&table.name)
            .cloned()
            .ok_or_else(|| Error::Config(format!("table `{}` sans ressource", table.name)))?;
        for column in table.columns.iter().filter(|c| c.ty == ColumnType::Enum) {
            let values = column.values.as_deref().unwrap_or_default();
            builder = builder.register(
                Enum::new(names::enumeration(&table.name, &column.name))
                    .items(values.iter().map(String::as_str)),
            );
        }
        builder = builder
            .register(record_type(model, table, locale))
            .register(page_type(table))
            .register(input_type(table))
            .register(filter_type(table));
        query = query
            .field(read_field(table, service.clone()))
            .field(list_field(table, service.clone()))
            .field(aggregate_field(table, service.clone()));
        mutation = mutation
            .field(create_field(table, service.clone()))
            .field(update_field(table, service.clone()))
            .field(delete_field(table, service));
    }
    query = query.field(parameters_field());
    mutation = mutation.field(update_parameter_field());

    for field in extensions.queries {
        query = query.field(field);
    }
    for field in extensions.mutations {
        mutation = mutation.field(field);
    }
    for ty in extensions.types {
        builder = builder.register(ty);
    }
    builder
        .register(query)
        .register(mutation)
        .finish()
        .map_err(|err| Error::Config(format!("schéma GraphQL : {err}")))
}

/// Scalaires de forge et filtres par nature de valeur.
fn builtin_types(mut builder: SchemaBuilder) -> SchemaBuilder {
    let scalars = [
        (
            "Decimal",
            "Nombre décimal exact, en texte : \"12.50\" (un nombre est accepté en entrée).",
        ),
        ("Date", "Date au format AAAA-MM-JJ."),
        ("DateTime", "Date-heure RFC 3339 : 2026-01-31T14:00:00Z."),
        ("JSON", "Valeur JSON quelconque."),
    ];
    for (name, description) in scalars {
        builder = builder.register(Scalar::new(name).description(description));
    }
    builder = builder.register(file_type());
    for kind in KINDS {
        let scalar = kind.scalar();
        let mut filter = InputObject::new(kind.filter())
            .field(InputValue::new("eq", TypeRef::named(scalar)))
            .field(InputValue::new("ne", TypeRef::named(scalar)))
            .field(InputValue::new("in", TypeRef::named_nn_list(scalar)))
            .field(InputValue::new("is_null", TypeRef::named(TypeRef::BOOLEAN)));
        if !matches!(kind, Kind::Id | Kind::Boolean) {
            for op in ["lt", "lte", "gt", "gte"] {
                filter = filter.field(InputValue::new(op, TypeRef::named(scalar)));
            }
        }
        if kind == Kind::String {
            filter = filter.field(
                InputValue::new("like", TypeRef::named(scalar))
                    .description("Contient, sans tenir compte de la casse."),
            );
        }
        builder = builder.register(filter);
    }
    builder
}

fn label(label: Option<&Label>, locale: &str) -> Option<String> {
    match label? {
        Label::Plain(text) => Some(text.clone()),
        Label::Localized(map) => map.get(locale).cloned(),
    }
}

// ------------------------------------------------------------------ types

fn record_type(model: &Model, table: &Table, locale: &str) -> Object {
    let mut object =
        Object::new(names::object(&table.name)).field(scalar_field("id", Kind::Id, true));
    if let Some(text) = label(table.label.as_ref(), locale) {
        object = object.description(text);
    }
    for column in &table.columns {
        let mut field = match column.ty {
            ColumnType::Reference => reference_field(column),
            ColumnType::ReferenceList => reference_list_field(column),
            ColumnType::Enum => enum_field(&table.name, column),
            _ => match compute::result_type(model, &ColumnRef::new(&table.name, &column.name)) {
                ty if ty.is_file() => file_field(&column.name),
                ty => scalar_field(&column.name, Kind::of(ty), false),
            },
        };
        if let Some(text) = label(column.label.as_ref(), locale) {
            field = field.description(text);
        }
        object = object.field(field);
    }
    object
        .field(scalar_field("owner", Kind::Id, false).description("Créateur de l'enregistrement."))
        .field(scalar_field("created_at", Kind::DateTime, true))
        .field(scalar_field("updated_at", Kind::DateTime, true))
}

/// Enregistrement JSON porté par la valeur parente.
fn parent<'a>(ctx: &ResolverContext<'a>) -> GraphQlResult<&'a JsonValue> {
    ctx.parent_value.try_downcast_ref::<JsonValue>()
}

fn scalar_field(name: &str, kind: Kind, required: bool) -> Field {
    let ty = if required {
        TypeRef::named_nn(kind.scalar())
    } else {
        TypeRef::named(kind.scalar())
    };
    let key = name.to_owned();
    Field::new(name, ty, move |ctx| {
        let key = key.clone();
        FieldFuture::new(
            async move { Ok(kind.output(&parent(&ctx)?[&key]).map(FieldValue::value)) },
        )
    })
}

/// Fichier téléversé, tel que décrit par [`crate::files`].
fn file_type() -> Object {
    Object::new(FILE_TYPE)
        .description(
            "Fichier téléversé ; `url` : lien temporaire vers son contenu. \
             En écriture, une colonne fichier reçoit l'`id` renvoyé par `POST /api/files`.",
        )
        .field(scalar_field("id", Kind::String, true))
        .field(scalar_field("name", Kind::String, true))
        .field(scalar_field("size", Kind::Int, true))
        .field(scalar_field("content_type", Kind::String, true))
        .field(scalar_field("url", Kind::String, true))
}

const FILE_TYPE: &str = "File";

fn file_field(name: &str) -> Field {
    let key = name.to_owned();
    Field::new(name, TypeRef::named(FILE_TYPE), move |ctx| {
        let key = key.clone();
        FieldFuture::new(async move {
            let file = &parent(&ctx)?[&key];
            Ok((!file.is_null()).then(|| FieldValue::owned_any(file.clone())))
        })
    })
}

fn enum_field(table: &str, column: &Column) -> Field {
    let key = column.name.clone();
    let ty = TypeRef::named(names::enumeration(table, &column.name));
    Field::new(&column.name, ty, move |ctx| {
        let key = key.clone();
        FieldFuture::new(async move {
            let value = parent(&ctx)?[&key]
                .as_str()
                .map(|s| Value::Enum(Name::new(s)));
            Ok(value.map(FieldValue::value))
        })
    })
}

/// Table cible d'une colonne de relation (validée par le schéma).
fn target(column: &Column) -> String {
    column.target.clone().unwrap_or_default()
}

/// Référence → enregistrement cible, chargé par lots ; `null` s'il est hors du
/// périmètre de lecture.
fn reference_field(column: &Column) -> Field {
    let (key, target) = (column.name.clone(), target(column));
    let ty = TypeRef::named(names::object(&target));
    Field::new(&column.name, ty, move |ctx| {
        let (key, target) = (key.clone(), target.clone());
        FieldFuture::new(async move {
            let Some(id) = parent(&ctx)?[&key].as_i64() else {
                return Ok(None);
            };
            let loader = ctx.data::<DataLoader<RecordLoader>>()?;
            let record = loader
                .load_one((target, id))
                .await
                .map_err(|err| to_graphql(&err))?;
            Ok(record.map(FieldValue::owned_any))
        })
    })
}

fn reference_list_field(column: &Column) -> Field {
    let (key, target) = (column.name.clone(), target(column));
    let ty = TypeRef::named_nn_list_nn(names::object(&target));
    Field::new(&column.name, ty, move |ctx| {
        let (key, target) = (key.clone(), target.clone());
        FieldFuture::new(async move {
            let ids: Vec<i64> = parent(&ctx)?[&key]
                .as_array()
                .map(|ids| ids.iter().filter_map(JsonValue::as_i64).collect())
                .unwrap_or_default();
            let loader = ctx.data::<DataLoader<RecordLoader>>()?;
            let mut records = loader
                .load_many(ids.iter().map(|id| (target.clone(), *id)))
                .await
                .map_err(|err| to_graphql(&err))?;
            let ordered = ids
                .iter()
                .filter_map(|id| records.remove(&(target.clone(), *id)))
                .map(FieldValue::owned_any);
            Ok(Some(FieldValue::list(ordered)))
        })
    })
}

fn page_type(table: &Table) -> Object {
    let count = |name: &'static str, get: fn(&Listing) -> u64| {
        Field::new(name, TypeRef::named_nn(TypeRef::INT), move |ctx| {
            FieldFuture::new(async move {
                let page = ctx.parent_value.try_downcast_ref::<Listing>()?;
                Ok(Some(FieldValue::value(get(page))))
            })
        })
    };
    let data = Field::new(
        "data",
        TypeRef::named_nn_list_nn(names::object(&table.name)),
        |ctx| {
            FieldFuture::new(async move {
                let page = ctx.parent_value.try_downcast_ref::<Listing>()?;
                let records = page.data.iter().cloned().map(FieldValue::owned_any);
                Ok(Some(FieldValue::list(records)))
            })
        },
    );
    Object::new(names::page(&table.name))
        .field(data)
        .field(count("page", |p| p.page))
        .field(count("per_page", |p| p.per_page))
        .field(count("total", |p| p.total))
}

/// Champs modifiables, tous facultatifs : en modification, seuls les champs
/// fournis changent (`null` efface la valeur).
fn input_type(table: &Table) -> InputObject {
    let mut input = InputObject::new(names::input(&table.name));
    for column in table.columns.iter().filter(|c| !c.is_computed()) {
        let ty = match column.ty {
            ColumnType::ReferenceList => TypeRef::named_nn_list(TypeRef::ID),
            ColumnType::Enum => TypeRef::named(names::enumeration(&table.name, &column.name)),
            ty => TypeRef::named(Kind::of(ty).scalar()),
        };
        input = input.field(InputValue::new(&column.name, ty));
    }
    input
}

/// Un filtre par colonne stockée : `{ montant: { gte: "1000" }, etape: { in: ["gagne"] } }`.
fn filter_type(table: &Table) -> InputObject {
    let system = [
        ("id", Kind::Id),
        ("owner", Kind::Id),
        ("created_at", Kind::DateTime),
        ("updated_at", Kind::DateTime),
    ];
    let stored = table
        .columns
        .iter()
        .filter(|c| c.is_stored() && c.ty != ColumnType::ReferenceList)
        .map(|c| (c.name.as_str(), Kind::of(c.ty)));
    system.into_iter().chain(stored).fold(
        InputObject::new(names::filter(&table.name)),
        |input, (name, kind)| input.field(InputValue::new(name, TypeRef::named(kind.filter()))),
    )
}

// ------------------------------------------------------------------ opérations

/// État de l'application et utilisateur de la requête.
fn context<'a>(ctx: &ResolverContext<'a>) -> GraphQlResult<(&'a AppState, &'a CurrentUser)> {
    Ok((ctx.data::<AppState>()?, ctx.data::<CurrentUser>()?))
}

/// Identifiant passé en argument (`ID` : texte ou entier).
fn id_of(value: &Value) -> Result<i64, Error> {
    let id = match value {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    };
    id.ok_or_else(|| Error::validation("id", "identifiant entier attendu"))
}

fn id_argument(ctx: &ResolverContext<'_>) -> GraphQlResult<i64> {
    id_of(ctx.args.try_get("id")?.as_value()).map_err(|err| to_graphql(&err))
}

fn read_field(table: &Table, service: Arc<dyn Service>) -> Field {
    Field::new(
        &table.name,
        TypeRef::named(names::object(&table.name)),
        move |ctx| {
            let service = service.clone();
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                match service.read(state, user, id_argument(&ctx)?).await {
                    Ok(record) => Ok(Some(FieldValue::owned_any(record))),
                    Err(Error::NotFound) => Ok(None),
                    Err(err) => Err(to_graphql(&err)),
                }
            })
        },
    )
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
}

/// Valeur d'argument → texte d'un paramètre de liste (`a,b` pour une liste).
fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Enum(name) => name.to_string(),
        Value::List(items) => items.iter().map(text).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

/// Arguments de liste → paires de requête REST (`montant[gte]=1000`), validées
/// ensuite comme une URL : mêmes règles et mêmes messages d'erreur.
fn query_pairs(args: &ObjectAccessor<'_>) -> GraphQlResult<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for key in ["page", "per_page", "sort", "q", "fields", "group_by"] {
        if let Some(value) = args.get(key).filter(|v| !v.is_null()) {
            pairs.push((key.to_owned(), text(value.as_value())));
        }
    }
    if let Some(filter) = args.get("filter").filter(|v| !v.is_null()) {
        for (column, ops) in filter.object()?.iter() {
            if ops.is_null() {
                continue;
            }
            for (op, value) in ops.object()?.iter().filter(|(_, v)| !v.is_null()) {
                let key = match op.as_str() {
                    "eq" => column.to_string(),
                    "is_null" => format!("{column}[null]"),
                    op => format!("{column}[{op}]"),
                };
                pairs.push((key, text(value.as_value())));
            }
        }
    }
    Ok(pairs)
}

fn filter_arguments(field: Field, table: &Table) -> Field {
    field
        .argument(
            InputValue::new("q", TypeRef::named(TypeRef::STRING))
                .description("Recherche dans les colonnes texte."),
        )
        .argument(InputValue::new(
            "filter",
            TypeRef::named(names::filter(&table.name)),
        ))
}

fn list_field(table: &Table, service: Arc<dyn Service>) -> Field {
    let field = Field::new(
        names::list(&table.name),
        TypeRef::named_nn(names::page(&table.name)),
        move |ctx| {
            let service = service.clone();
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                let table = state.table(service.name());
                let query = ListQuery::from_pairs(table, query_pairs(&ctx.args)?)
                    .map_err(|err| to_graphql(&err))?;
                let page = service
                    .list(state, user, &query)
                    .await
                    .map_err(|err| to_graphql(&err))?;
                Ok(Some(FieldValue::owned_any(page)))
            })
        },
    )
    .argument(InputValue::new("page", TypeRef::named(TypeRef::INT)))
    .argument(InputValue::new("per_page", TypeRef::named(TypeRef::INT)))
    .argument(
        InputValue::new("sort", TypeRef::named(TypeRef::STRING)).description(
            "Colonnes séparées par des virgules, `-` pour décroissant : `-montant,titre`.",
        ),
    );
    filter_arguments(field, table)
}

fn aggregate_field(table: &Table, service: Arc<dyn Service>) -> Field {
    let field = Field::new(names::aggregate(&table.name), TypeRef::named_nn("JSON"), move |ctx| {
        let service = service.clone();
        FieldFuture::new(async move {
            let (state, user) = context(&ctx)?;
            let table = state.table(service.name());
            let query = AggregateQuery::from_pairs(table, query_pairs(&ctx.args)?).map_err(|err| to_graphql(&err))?;
            let result = service.aggregate(state, user, &query).await.map_err(|err| to_graphql(&err))?;
            Ok(Some(FieldValue::value(Value::from_json(result)?)))
        })
    })
    .description("Nombre, somme, moyenne, minimum et maximum, au total et par groupe (comme `GET /api/<table>/aggregate`).")
    .argument(InputValue::new("fields", TypeRef::named_nn_list_nn(TypeRef::STRING)))
    .argument(InputValue::new("group_by", TypeRef::named(TypeRef::STRING)));
    filter_arguments(field, table)
}

/// Argument `data` → corps JSON, comme celui de l'API REST.
fn body(table: &Table, data: &ValueAccessor<'_>) -> Result<JsonValue, Error> {
    let data = data
        .object()
        .map_err(|err| Error::BadRequest(err.message))?;
    let mut body = serde_json::Map::new();
    for (name, value) in data.iter() {
        let ty = table
            .columns
            .iter()
            .find(|c| c.name == name.as_str())
            .map(|c| c.ty);
        let json = match (ty, value.as_value()) {
            (_, Value::Null) => JsonValue::Null,
            (Some(ColumnType::Reference), id) => id_of(id)
                .map_err(|_| Error::validation(name.as_str(), "identifiant entier attendu"))?
                .into(),
            (Some(ColumnType::ReferenceList), Value::List(ids)) => ids
                .iter()
                .map(|id| id_of(id).map(JsonValue::from))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::validation(name.as_str(), "identifiants entiers attendus"))?
                .into(),
            (_, other) => other
                .clone()
                .into_json()
                .map_err(|err| Error::BadRequest(err.to_string()))?,
        };
        body.insert(name.to_string(), json);
    }
    Ok(JsonValue::Object(body))
}

fn data_argument(ctx: &ResolverContext<'_>, table: &Table) -> GraphQlResult<JsonValue> {
    body(table, &ctx.args.try_get("data")?).map_err(|err| to_graphql(&err))
}

fn create_field(table: &Table, service: Arc<dyn Service>) -> Field {
    Field::new(
        names::create(&table.name),
        TypeRef::named_nn(names::object(&table.name)),
        move |ctx| {
            let service = service.clone();
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                let body = data_argument(&ctx, state.table(service.name()))?;
                let record = service
                    .create(state, user, body)
                    .await
                    .map_err(|err| to_graphql(&err))?;
                Ok(Some(FieldValue::owned_any(record)))
            })
        },
    )
    .argument(InputValue::new(
        "data",
        TypeRef::named_nn(names::input(&table.name)),
    ))
}

fn update_field(table: &Table, service: Arc<dyn Service>) -> Field {
    Field::new(
        names::update(&table.name),
        TypeRef::named_nn(names::object(&table.name)),
        move |ctx| {
            let service = service.clone();
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                let id = id_argument(&ctx)?;
                let body = data_argument(&ctx, state.table(service.name()))?;
                let record = service
                    .update(state, user, id, body)
                    .await
                    .map_err(|err| to_graphql(&err))?;
                Ok(Some(FieldValue::owned_any(record)))
            })
        },
    )
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
    .argument(InputValue::new(
        "data",
        TypeRef::named_nn(names::input(&table.name)),
    ))
}

fn delete_field(table: &Table, service: Arc<dyn Service>) -> Field {
    Field::new(
        names::delete(&table.name),
        TypeRef::named_nn(TypeRef::BOOLEAN),
        move |ctx| {
            let service = service.clone();
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                service
                    .delete(state, user, id_argument(&ctx)?)
                    .await
                    .map_err(|err| to_graphql(&err))?;
                Ok(Some(FieldValue::value(true)))
            })
        },
    )
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
}

fn parameters_field() -> Field {
    Field::new(names::PARAMETERS_QUERY, TypeRef::named_nn("JSON"), |ctx| {
        FieldFuture::new(async move {
            let (state, _) = context(&ctx)?;
            let params = parameters::all(state)
                .await
                .map_err(|err| to_graphql(&err))?;
            Ok(Some(FieldValue::value(Value::from_json(params)?)))
        })
    })
    .description("Paramètres et leurs valeurs (comme `GET /api/parameters`).")
}

fn update_parameter_field() -> Field {
    Field::new(
        names::PARAMETER_MUTATION,
        TypeRef::named_nn("JSON"),
        |ctx| {
            FieldFuture::new(async move {
                let (state, user) = context(&ctx)?;
                let name = ctx.args.try_get("name")?.string()?;
                let value = ctx
                    .args
                    .get("value")
                    .map_or(Ok(JsonValue::Null), |v| v.as_value().clone().into_json())?;
                let param = parameters::set(state, user, name, value)
                    .await
                    .map_err(|err| to_graphql(&err))?;
                Ok(Some(FieldValue::value(Value::from_json(param)?)))
            })
        },
    )
    .description("Modifie un paramètre (administrateur).")
    .argument(InputValue::new("name", TypeRef::named_nn(TypeRef::STRING)))
    .argument(InputValue::new("value", TypeRef::named("JSON")))
}
