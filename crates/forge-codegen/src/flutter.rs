//! Fichiers de l'application Flutter (`app/`) : description des tables et
//! modèles typés (réécrits), point d'entrée et personnalisation (créés une fois).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use forge_schema::Model;
use forge_schema::names::pascal_case;
use forge_schema::spec::{Column, ColumnType, Label, Table};
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::dart::{Expr, call, camel_case, named, pos, quote, raw, statement, string};
use crate::error::Error;
use crate::frontend::{field_options, operations};
use crate::render::Renderer;
use crate::writer::{OutputFile, Policy};

/// Dossiers entièrement gérés par
pub(crate) const GENERATED_DIRS: &[&str] = &["app/lib/generated"];

const HEADER: &str = "// NE PAS MODIFIER : code généré par forge depuis `forge.json`.\n\
                      // Ce dossier est réécrit à chaque `forge generate` ; votre code va dans\n\
                      // `lib/custom/`.\n";

/// Types de `dart:core` qu'une classe de modèle ne doit pas masquer.
const DART_CORE_TYPES: &[&str] = &[
    "BigInt",
    "Comparable",
    "DateTime",
    "Deprecated",
    "Duration",
    "Enum",
    "Error",
    "Exception",
    "Expando",
    "Finalizer",
    "Function",
    "Future",
    "Invocation",
    "Iterable",
    "Iterator",
    "List",
    "Map",
    "MapEntry",
    "Match",
    "Never",
    "Null",
    "Object",
    "Pattern",
    "Record",
    "RegExp",
    "Runes",
    "Set",
    "Sink",
    "StackTrace",
    "Stopwatch",
    "Stream",
    "String",
    "StringBuffer",
    "StringSink",
    "Symbol",
    "Type",
    "Uri",
    "WeakReference",
];

/// Noms de `forge_flutter` utilisables par `models.dart`, importés sans préfixe
/// (d'où le renommage des tables homonymes de ces types) et seulement s'ils
/// servent.
const IMPORTABLE: &[&str] = &[
    "ForgeClient",
    "ForgeFile",
    "TableClient",
    "dateTimeToJson",
    "dateToJson",
    "decimalToJson",
    "durationToJson",
    "jsonToDate",
    "jsonToDateTime",
    "jsonToDecimal",
    "jsonToDuration",
    "jsonToIds",
];

/// `name` apparaît-il comme identifiant entier dans `code` ?
fn uses(code: &str, name: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    code.match_indices(name).any(|(i, _)| {
        !code[..i].ends_with(is_ident) && !code[i + name.len()..].starts_with(is_ident)
    })
}

/// Imports de `models.dart`, mis en forme comme par `dart format`.
fn model_imports(body: &str) -> String {
    let mut out = String::new();
    if uses(body, "Decimal") {
        out.push_str("import 'package:decimal/decimal.dart';\n");
    }
    let names: Vec<&str> = IMPORTABLE
        .iter()
        .copied()
        .filter(|name| uses(body, name))
        .collect();
    out.push_str("import 'package:forge_flutter/forge_flutter.dart'");
    let line = format!("    show {};", names.join(", "));
    if line.len() <= 80 {
        let _ = write!(out, "\n{line}\n");
    } else {
        out.push_str("\n    show\n");
        let _ = writeln!(out, "        {};", names.join(",\n        "));
    }
    out
}

/// Types importés par `models.dart`.
const IMPORTED_TYPES: &[&str] = &["Decimal", "ForgeClient", "ForgeFile", "TableClient"];

/// Noms déjà pris dans une classe de modèle ou une énumération.
const MEMBERS: &[&str] = &[
    "api",
    "fromJson",
    "hashCode",
    "index",
    "name",
    "noSuchMethod",
    "runtimeType",
    "table",
    "toJson",
    "toString",
    "value",
    "values",
];

/// Mots réservés de Dart (une valeur d'énumération devient une constante).
const DART_RESERVED: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
    "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
    "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

pub(crate) struct Flutter<'a> {
    pub model: &'a Model,
    /// Chemin du package `forge_flutter`, relatif au dossier `app/`.
    pub package_path: &'a str,
}

#[derive(Serialize)]
struct TableView {
    name: String,
    class_name: String,
    /// Déclaration `final json = {...};` d'un enregistrement d'exemple.
    sample: String,
}

impl Flutter<'_> {
    pub(crate) fn files(&self, renderer: &Renderer) -> Result<Vec<OutputFile>, Error> {
        let spec = self.model.spec();
        let names = Names::new(self.model);
        let tables: Vec<TableView> = spec
            .tables
            .iter()
            .map(|table| TableView {
                name: table.name.clone(),
                class_name: Names::class_of(&table.name),
                sample: statement(4, "final json = ", &self.sample(table), ";"),
            })
            .collect();
        let context = serde_json::json!({
            "app_name": spec.app.name,
            "package_name": format!("{}_app", spec.app.name),
            "default_locale": spec.app.default_locale,
            "package_path": self.package_path,
            "tables": tables,
        });
        let file = |path: &str, template: &str, policy: Policy| -> Result<OutputFile, Error> {
            Ok(OutputFile {
                path: PathBuf::from(path),
                content: renderer.render(template, &context)?,
                policy,
            })
        };
        Ok(vec![
            file("app/pubspec.yaml", "flutter/pubspec.yaml", Policy::Once)?,
            file(
                "app/analysis_options.yaml",
                "flutter/analysis_options.yaml",
                Policy::Once,
            )?,
            file("app/.gitignore", "flutter/gitignore", Policy::Once)?,
            file("app/lib/main.dart", "flutter/main.dart", Policy::Once)?,
            file(
                "app/lib/custom/customization.dart",
                "flutter/customization.dart",
                Policy::Once,
            )?,
            file(
                "app/lib/custom/theme.dart",
                "flutter/theme.dart",
                Policy::Once,
            )?,
            file("app/web/index.html", "flutter/index.html", Policy::Once)?,
            file(
                "app/web/manifest.json",
                "flutter/manifest.json",
                Policy::Once,
            )?,
            file(
                "app/lib/generated/app.dart",
                "flutter/app.dart",
                Policy::Generated,
            )?,
            file(
                "app/test/generated_test.dart",
                "flutter/generated_test.dart",
                Policy::Generated,
            )?,
            OutputFile {
                path: PathBuf::from("app/lib/generated/schema.dart"),
                content: self.schema(),
                policy: Policy::Generated,
            },
            OutputFile {
                path: PathBuf::from("app/lib/generated/models.dart"),
                content: self.models(&names),
                policy: Policy::Generated,
            },
        ])
    }

    // ------------------------------------------------------------ schema.dart

    fn schema(&self) -> String {
        let spec = self.model.spec();
        let strings = |items: &[String]| Expr::List(items.iter().map(|s| string(s)).collect());
        let mut args = vec![
            named("name", string(&spec.app.name)),
            named("defaultLocale", string(&spec.app.default_locale)),
            named("locales", strings(&spec.app.locales)),
            named("roles", strings(&spec.roles)),
        ];
        if !spec.parameters.is_empty() {
            let parameters = spec
                .parameters
                .iter()
                .map(|p| {
                    let mut args = vec![pos(string(&p.name)), pos(column_type(p.ty))];
                    args.extend(p.label.as_ref().map(|l| named("label", label(l))));
                    call("Parameter", args)
                })
                .collect();
            args.push(named("parameters", Expr::List(parameters)));
        }
        args.push(named(
            "tables",
            Expr::List(spec.tables.iter().map(|t| self.table(t)).collect()),
        ));
        format!(
            "{HEADER}\nimport 'package:forge_flutter/forge_flutter.dart';\n\n\
             /// Description des tables de l'application, lue par l'interface.\n{}",
            statement(0, "const schema = ", &call("AppSchema", args), ";")
        )
    }

    fn table(&self, table: &Table) -> Expr {
        let mut args = vec![pos(string(&table.name))];
        args.extend(table.label.as_ref().map(|l| named("label", label(l))));
        args.push(named(
            "columns",
            Expr::List(
                table
                    .columns
                    .iter()
                    .map(|c| self.column(&table.name, c))
                    .collect(),
            ),
        ));
        if let Some(view) = &table.views.calendar {
            let mut view_args = vec![named("start", string(&view.start))];
            view_args.extend(view.end.as_deref().map(|e| named("end", string(e))));
            view_args.extend(
                view.duration
                    .as_deref()
                    .map(|d| named("duration", string(d))),
            );
            args.push(named("calendar", call("CalendarView", view_args)));
        }
        if let Some(view) = &table.views.stats {
            let mut view_args = vec![named(
                "fields",
                Expr::List(view.fields.iter().map(|f| string(f)).collect()),
            )];
            view_args.extend(
                view.group_by
                    .as_deref()
                    .map(|g| named("groupBy", string(g))),
            );
            args.push(named("stats", call("StatsView", view_args)));
        }
        if !table.rules.is_empty() {
            let rules = table
                .rules
                .iter()
                .map(|rule| {
                    let mut rule_args = vec![
                        pos(Expr::List(rule.roles.iter().map(|r| string(r)).collect())),
                        pos(Expr::Set(
                            operations(rule)
                                .into_iter()
                                .map(|o| raw(format!("Operation.{o}")))
                                .collect(),
                        )),
                    ];
                    if rule.when.is_some() {
                        rule_args.push(named("conditional", raw("true")));
                    }
                    call("Rule", rule_args)
                })
                .collect();
            args.push(named("rules", Expr::List(rules)));
        }
        call("TableSchema", args)
    }

    fn column(&self, table: &str, column: &Column) -> Expr {
        let (_, shown) = self.model.resolved(table, column);
        let mut args = vec![pos(string(&column.name)), pos(column_type(shown.ty))];
        args.extend(column.label.as_ref().map(|l| named("label", label(l))));
        let flag = |name: &str, on: bool| on.then(|| named(name, raw("true")));
        args.extend(flag("required", column.required));
        args.extend(flag("unique", column.unique));
        args.extend(flag("hidden", column.hidden));
        args.extend(flag("titleField", column.title_field));
        if let Some(default) = &column.default {
            args.push(named("defaultValue", json_literal(default)));
        }
        args.extend(flag("computed", column.is_computed()));
        if !column.is_stored() {
            args.push(named("stored", raw("false")));
        }
        if let Some(target) = &shown.target {
            args.push(named("target", string(target)));
        }
        if let Some(values) = &shown.values {
            args.push(named(
                "values",
                Expr::List(values.iter().map(|v| string(v)).collect()),
            ));
        }
        let options = field_options(self.model, shown);
        args.extend(options.max.map(|max| named("max", raw(max.to_string()))));
        args.extend(options.currency.map(|c| named("currency", string(c))));
        args.extend(
            options
                .max_size
                .map(|size| named("maxSize", raw(size.to_string()))),
        );
        if !options.accept.is_empty() {
            args.push(named(
                "accept",
                Expr::List(options.accept.iter().map(|a| string(a)).collect()),
            ));
        }
        call("ColumnSchema", args)
    }

    // ------------------------------------------------------------ models.dart

    fn models(&self, names: &Names) -> String {
        let example = self
            .model
            .tables()
            .first()
            .map_or_else(String::new, |t| Names::class_of(&t.name));
        let mut body = String::new();
        // Une énumération par colonne `enum` (les lookups réutilisent celle de
        // la colonne lue).
        let mut enums = String::new();
        for table in &self.model.spec().tables {
            body.push('\n');
            body.push_str(&self.model_class(table, names));
            for column in &table.columns {
                if column.ty == ColumnType::Enum {
                    enums.push('\n');
                    enums.push_str(&enumeration(
                        &names.enumeration(&table.name, &column.name),
                        column.values.as_deref().unwrap_or_default(),
                    ));
                }
            }
        }
        body += &enums;
        format!(
            "{HEADER}\n// Modèles typés des tables, pour le code personnalisé :\n\
             //   final listing = await {example}.api(client).list();\n\n\
             {}{body}",
            model_imports(&body)
        )
    }

    fn model_class(&self, table: &Table, names: &Names) -> String {
        let class = Names::class_of(&table.name);
        let fields = self.fields(table, names);
        let label = table
            .label
            .as_ref()
            .and_then(|l| match l {
                Label::Plain(text) => Some(text.clone()),
                Label::Localized(map) => map.get(&self.model.spec().app.default_locale).cloned(),
            })
            .unwrap_or_else(|| table.name.clone());
        let mut out = format!("/// {label} (table `{}`).\nclass {class} {{\n", table.name);

        let params: Vec<String> = fields
            .iter()
            .map(|f| {
                let required = if f.nullable { "" } else { "required " };
                format!("{required}this.{}", f.dart)
            })
            .collect();
        out += &parameter_list(&format!("  const {class}("), &params, ");");

        let decode = call(
            class.clone(),
            fields
                .iter()
                .map(|f| named(&f.dart, raw(&f.decode)))
                .collect(),
        );
        out += "\n";
        out += &statement(
            2,
            &format!("factory {class}.fromJson(Map<String, dynamic> json) => "),
            &decode,
            ";",
        );

        let _ = write!(
            out,
            "\n  static const table = {};\n\n  /// Opérations typées sur la table.\n",
            quote(&table.name)
        );
        let client = call(
            "TableClient",
            vec![
                pos(raw("client")),
                pos(raw("table")),
                pos(raw(format!("{class}.fromJson"))),
            ],
        );
        out += &statement(
            2,
            &format!("static TableClient<{class}> api(ForgeClient client) => "),
            &client,
            ";",
        );

        out += "\n";
        for field in &fields {
            let q = if field.nullable { "?" } else { "" };
            let _ = writeln!(out, "  final {}{q} {};", field.ty, field.dart);
        }

        let entries: Vec<(Expr, Expr)> = fields
            .iter()
            .filter_map(|f| Some((string(&f.json), raw(f.encode.clone()?))))
            .collect();
        out += "\n  /// Colonnes modifiables, au format de l'API (création, modification).\n";
        out += &statement(
            2,
            "Map<String, dynamic> toJson() => ",
            &Expr::Map(entries),
            ";",
        );
        out + "}\n"
    }

    /// Champs du modèle : identifiant, colonnes, puis colonnes système.
    fn fields(&self, table: &Table, names: &Names) -> Vec<Field> {
        let system = |json: &str, ty: &str, nullable: bool, decode: String| Field {
            json: json.into(),
            dart: camel_case(json),
            ty: ty.into(),
            nullable,
            decode,
            encode: None,
        };
        let mut fields = vec![system("id", "int", false, "json['id'] as int".into())];
        fields.extend(table.columns.iter().map(|c| self.field(table, c, names)));
        fields.push(system("owner", "int", true, "json['owner'] as int?".into()));
        for name in ["created_at", "updated_at"] {
            let decode = format!("jsonToDateTime(json['{name}'])!");
            fields.push(system(name, "DateTime", false, decode));
        }
        fields
    }

    fn field(&self, table: &Table, column: &Column, names: &Names) -> Field {
        let (owner, shown) = self.model.resolved(&table.name, column);
        let dart = Names::field(&column.name);
        let key = format!("json[{}]", quote(&column.name));
        // Une liste vide remplace `null` ; une colonne requise saisie n'est jamais nulle.
        let nullable =
            (!column.required || column.is_computed()) && shown.ty != ColumnType::ReferenceList;
        let bang = if nullable { "" } else { "!" };
        let q = if nullable { "?" } else { "" };
        // Un modèle de champ prend le type Dart de son type de base, sauf les fichiers.
        let (ty, decode, encode) = match (shown.ty, shown.ty.base()) {
            (ColumnType::File | ColumnType::Image, _) => (
                "ForgeFile".into(),
                format!("ForgeFile.fromJson({key}){bang}"),
                format!("{dart}{q}.id"),
            ),
            (_, ColumnType::String | ColumnType::Text) => {
                ("String".into(), format!("{key} as String{q}"), dart.clone())
            }
            (_, ColumnType::Integer | ColumnType::Reference) => {
                ("int".into(), format!("{key} as int{q}"), dart.clone())
            }
            (_, ColumnType::Boolean) => ("bool".into(), format!("{key} as bool{q}"), dart.clone()),
            (_, ColumnType::Decimal) => (
                "Decimal".into(),
                format!("jsonToDecimal({key}){bang}"),
                format!("decimalToJson({dart})"),
            ),
            (_, ColumnType::Date) => (
                "DateTime".into(),
                format!("jsonToDate({key}){bang}"),
                format!("dateToJson({dart})"),
            ),
            (_, ColumnType::Datetime) => (
                "DateTime".into(),
                format!("jsonToDateTime({key}){bang}"),
                format!("dateTimeToJson({dart})"),
            ),
            (_, ColumnType::Duration) => (
                "Duration".into(),
                format!("jsonToDuration({key}){bang}"),
                format!("durationToJson({dart})"),
            ),
            (_, ColumnType::Enum) => {
                let ty = names.enumeration(owner, &shown.name);
                let decode = format!("{ty}.fromJson({key}){bang}");
                (ty, decode, format!("{dart}{q}.value"))
            }
            (_, ColumnType::ReferenceList) => (
                "List<int>".into(),
                format!("jsonToIds({key})"),
                dart.clone(),
            ),
            _ => ("Object".into(), key, dart.clone()),
        };
        Field {
            json: column.name.clone(),
            dart,
            ty,
            nullable,
            decode,
            encode: (!column.is_computed()).then_some(encode),
        }
    }

    /// Enregistrement d'exemple au format de l'API, pour le test des modèles.
    fn sample(&self, table: &Table) -> Expr {
        let mut entries = vec![(string("id"), raw("1"))];
        for column in &table.columns {
            let (_, shown) = self.model.resolved(&table.name, column);
            let value = match (shown.ty, shown.ty.base()) {
                (ColumnType::File | ColumnType::Image, _) => Expr::Map(vec![
                    (string("id"), string("0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a")),
                    (string("name"), string("fichier.png")),
                    (string("size"), raw("1024")),
                    (string("content_type"), string("image/png")),
                    (string("url"), string("/api/files/0b9f3c1e")),
                ]),
                (_, ColumnType::String | ColumnType::Text) => string("texte"),
                (_, ColumnType::Integer | ColumnType::Reference) => raw("2"),
                (_, ColumnType::Decimal) => string("1234.5"),
                (_, ColumnType::Boolean) => raw("true"),
                (_, ColumnType::Date) => string("2026-01-02"),
                (_, ColumnType::Datetime) => string("2026-01-02T03:04:05.000Z"),
                (_, ColumnType::Duration) => raw("5400"),
                (_, ColumnType::Enum) => string(
                    shown
                        .values
                        .as_ref()
                        .and_then(|v| v.first())
                        .map_or("", String::as_str),
                ),
                (_, ColumnType::ReferenceList) => Expr::List(vec![raw("2"), raw("3")]),
                _ => raw("null"),
            };
            entries.push((string(&column.name), value));
        }
        entries.extend([
            (string("owner"), raw("1")),
            (string("created_at"), string("2026-01-02T03:04:05.000Z")),
            (string("updated_at"), string("2026-01-02T03:04:05.000Z")),
        ]);
        Expr::Map(entries)
    }
}

struct Field {
    json: String,
    dart: String,
    ty: String,
    nullable: bool,
    decode: String,
    /// Valeur envoyée à l'API ; `None` pour une colonne en lecture seule.
    encode: Option<String>,
}

/// Noms Dart des tables, colonnes et énumérations, sans conflit.
struct Names {
    classes: BTreeSet<String>,
}

impl Names {
    fn new(model: &Model) -> Self {
        Self {
            classes: model
                .tables()
                .iter()
                .map(|t| Self::class_of(&t.name))
                .collect(),
        }
    }

    fn class_of(table: &str) -> String {
        let name = pascal_case(table);
        if DART_CORE_TYPES.contains(&name.as_str()) || IMPORTED_TYPES.contains(&name.as_str()) {
            name + "Record"
        } else {
            name
        }
    }

    fn enumeration(&self, table: &str, column: &str) -> String {
        let name = Self::class_of(table) + &pascal_case(column);
        if self.classes.contains(&name)
            || DART_CORE_TYPES.contains(&name.as_str())
            || IMPORTED_TYPES.contains(&name.as_str())
        {
            name + "Value"
        } else {
            name
        }
    }

    fn field(column: &str) -> String {
        escape(camel_case(column))
    }
}

/// Évite les mots réservés et les membres des classes générées.
fn escape(name: String) -> String {
    if DART_RESERVED.contains(&name.as_str()) || MEMBERS.contains(&name.as_str()) {
        name + "Value"
    } else {
        name
    }
}

fn enumeration(name: &str, values: &[String]) -> String {
    let mut out = format!("enum {name} {{\n");
    for (i, value) in values.iter().enumerate() {
        let end = if i + 1 == values.len() { ";" } else { "," };
        let _ = writeln!(
            out,
            "  {}({}){end}",
            escape(camel_case(value)),
            quote(value)
        );
    }
    let _ = write!(
        out,
        "\n  const {name}(this.value);\n\n  /// Valeur dans l'API.\n  final String value;\n\n\
         \x20 static {name}? fromJson(Object? json) {{\n\
         \x20   for (final candidate in values) {{\n\
         \x20     if (candidate.value == json) return candidate;\n\
         \x20   }}\n\
         \x20   return null;\n\
         \x20 }}\n}}\n"
    );
    out
}

/// Liste de paramètres `prefix a, b end` sur une ligne, ou un par ligne.
fn parameter_list(prefix: &str, params: &[String], end: &str) -> String {
    let flat = format!("{prefix}{{{}}}{end}", params.join(", "));
    if flat.chars().count() <= 80 {
        return flat + "\n";
    }
    let mut out = format!("{prefix}{{\n");
    for param in params {
        let _ = writeln!(out, "    {param},");
    }
    out + &format!("  }}{end}\n")
}

fn column_type(ty: ColumnType) -> Expr {
    raw(match ty {
        ColumnType::Enum => "ColumnType.enumeration".to_owned(),
        ColumnType::ReferenceList => "ColumnType.referenceList".to_owned(),
        other => format!("ColumnType.{}", other.name()),
    })
}

fn label(label: &Label) -> Expr {
    match label {
        Label::Plain(text) => call("Label.plain", vec![pos(string(text))]),
        Label::Localized(map) => call(
            "Label",
            vec![pos(Expr::Map(
                map.iter().map(|(k, v)| (string(k), string(v))).collect(),
            ))],
        ),
    }
}

/// Valeur JSON simple (défaut d'une colonne) en littéral Dart.
fn json_literal(value: &JsonValue) -> Expr {
    match value {
        JsonValue::String(text) => string(text),
        JsonValue::Array(items) => Expr::List(items.iter().map(json_literal).collect()),
        other => raw(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_names_avoid_conflicts() {
        assert_eq!(Names::class_of("opportunite"), "Opportunite");
        assert_eq!(Names::class_of("duration"), "DurationRecord");
        assert_eq!(escape(camel_case("to_json")), "toJsonValue");
        assert_eq!(escape(camel_case("date_cloture")), "dateCloture");
        assert_eq!(escape(camel_case("default")), "defaultValue");
    }

    #[test]
    fn model_imports_list_only_used_names() {
        let body = "TableClient<Note> api(ForgeClient c) => …; String? titre;";
        assert_eq!(
            model_imports(body),
            "import 'package:forge_flutter/forge_flutter.dart'\n    show ForgeClient, TableClient;\n"
        );
        let body = "Decimal? montant; jsonToDecimal(x); ForgeClient; TableClient; \
                    ForgeFile; jsonToDate(y); dateToJson(z); jsonToIds(w);";
        let imports = model_imports(body);
        assert!(imports.starts_with("import 'package:decimal/decimal.dart';\n"));
        assert!(imports.contains("    show\n        ForgeClient,\n        ForgeFile,\n"));
        assert!(!uses("jsonToDateTime(x)", "jsonToDate"));
    }
}
