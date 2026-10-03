//! Fichiers de l'application web React (`web/`) : description des tables et
//! types des enregistrements (réécrits), point d'entrée et personnalisation
//! (créés une fois).

use std::fmt::Write as _;
use std::path::PathBuf;

use forge_schema::Model;
use forge_schema::names::pascal_case;
use forge_schema::spec::{Column, ColumnType, Label, Table};
use serde_json::{Map, Value as JsonValue, json};

use crate::dart::camel_case;
use crate::error::Error;
use crate::frontend::{operations, shown};
use crate::render::Renderer;
use crate::writer::{OutputFile, Policy};

/// Dossiers entièrement gérés par forge.
pub(crate) const GENERATED_DIRS: &[&str] = &["web/src/generated"];

const HEADER: &str = "// NE PAS MODIFIER : code généré par forge depuis `forge.json`.\n\
                      // Ce dossier est réécrit à chaque `forge generate` ; votre code va dans\n\
                      // `src/custom/`.\n";

/// Types globaux du navigateur et de TypeScript qu'un type de table ne doit pas masquer.
const GLOBAL_TYPES: &[&str] = &[
    "Array", "Blob", "Boolean", "Date", "Document", "Element", "Error", "Event", "File",
    "Function", "Image", "Map", "Node", "Number", "Object", "Promise", "Record", "Request",
    "Response", "Set", "String", "Symbol", "Text", "URL", "Window",
];

pub(crate) struct Web<'a> {
    pub model: &'a Model,
    /// Chemin du package `@forge/web`, relatif au dossier `web/`.
    pub package_path: &'a str,
}

impl Web<'_> {
    pub(crate) fn files(&self, renderer: &Renderer) -> Result<Vec<OutputFile>, Error> {
        let app = &self.model.spec().app;
        let context = json!({
            "app_name": app.name,
            "package_name": format!("{}-web", app.name.replace('_', "-")),
            "default_locale": app.default_locale,
            "package_path": self.package_path,
        });
        let file = |path: &str, template: &str, policy: Policy| -> Result<OutputFile, Error> {
            Ok(OutputFile {
                path: PathBuf::from(path),
                content: renderer.render(template, &context)?,
                policy,
            })
        };
        Ok(vec![
            file("web/package.json", "web/package.json", Policy::Once)?,
            file("web/vite.config.ts", "web/vite.config.ts", Policy::Once)?,
            file("web/tsconfig.json", "web/tsconfig.json", Policy::Once)?,
            file("web/index.html", "web/index.html", Policy::Once)?,
            file("web/.gitignore", "web/gitignore", Policy::Once)?,
            file("web/src/main.tsx", "web/main.tsx", Policy::Once)?,
            file(
                "web/src/custom/customization.tsx",
                "web/customization.tsx",
                Policy::Once,
            )?,
            file(
                "web/src/generated/app.tsx",
                "web/app.tsx",
                Policy::Generated,
            )?,
            file(
                "web/src/generated/app.test.tsx",
                "web/app_test.tsx",
                Policy::Generated,
            )?,
            file(
                "web/src/generated/test-setup.ts",
                "web/test_setup.ts",
                Policy::Generated,
            )?,
            OutputFile {
                path: PathBuf::from("web/src/generated/schema.ts"),
                content: self.schema(),
                policy: Policy::Generated,
            },
            OutputFile {
                path: PathBuf::from("web/src/generated/models.ts"),
                content: self.models(),
                policy: Policy::Generated,
            },
        ])
    }

    // ------------------------------------------------------------- schema.ts

    fn schema(&self) -> String {
        let spec = self.model.spec();
        let mut app = Map::new();
        app.insert("name".into(), json!(spec.app.name));
        app.insert("default_locale".into(), json!(spec.app.default_locale));
        app.insert("locales".into(), json!(spec.app.locales));
        app.insert("roles".into(), json!(spec.roles));
        if !spec.parameters.is_empty() {
            let parameters: Vec<JsonValue> = spec
                .parameters
                .iter()
                .map(|p| {
                    let mut param = Map::new();
                    param.insert("name".into(), json!(p.name));
                    param.insert("type".into(), json!(p.ty.name()));
                    if let Some(label) = &p.label {
                        param.insert("label".into(), label_json(label));
                    }
                    JsonValue::Object(param)
                })
                .collect();
            app.insert("parameters".into(), JsonValue::Array(parameters));
        }
        let tables: Vec<JsonValue> = spec.tables.iter().map(|t| self.table(t)).collect();
        app.insert("tables".into(), JsonValue::Array(tables));
        let body = serde_json::to_string_pretty(&JsonValue::Object(app)).expect("JSON");
        format!(
            "{HEADER}\nimport type {{ AppSchema }} from '@forge/web';\n\n\
             /** Description des tables de l'application, lue par l'interface. */\n\
             export const schema: AppSchema = {body};\n"
        )
    }

    fn table(&self, table: &Table) -> JsonValue {
        let mut out = Map::new();
        out.insert("name".into(), json!(table.name));
        if let Some(label) = &table.label {
            out.insert("label".into(), label_json(label));
        }
        let columns: Vec<JsonValue> = table
            .columns
            .iter()
            .map(|c| self.column(&table.name, c))
            .collect();
        out.insert("columns".into(), JsonValue::Array(columns));
        if let Some(view) = &table.views.calendar {
            let mut calendar = Map::new();
            calendar.insert("start".into(), json!(view.start));
            if let Some(end) = &view.end {
                calendar.insert("end".into(), json!(end));
            }
            if let Some(duration) = &view.duration {
                calendar.insert("duration".into(), json!(duration));
            }
            out.insert("calendar".into(), JsonValue::Object(calendar));
        }
        if let Some(view) = &table.views.stats {
            let mut stats = Map::new();
            stats.insert("fields".into(), json!(view.fields));
            if let Some(group_by) = &view.group_by {
                stats.insert("group_by".into(), json!(group_by));
            }
            out.insert("stats".into(), JsonValue::Object(stats));
        }
        if !table.rules.is_empty() {
            let rules: Vec<JsonValue> = table
                .rules
                .iter()
                .map(|rule| {
                    let mut out = Map::new();
                    out.insert("roles".into(), json!(rule.roles));
                    out.insert("operations".into(), json!(operations(rule)));
                    if rule.when.is_some() {
                        out.insert("conditional".into(), json!(true));
                    }
                    JsonValue::Object(out)
                })
                .collect();
            out.insert("rules".into(), JsonValue::Array(rules));
        }
        JsonValue::Object(out)
    }

    fn column(&self, table: &str, column: &Column) -> JsonValue {
        let (_, displayed) = shown(self.model, table, column);
        let mut out = Map::new();
        out.insert("name".into(), json!(column.name));
        out.insert("type".into(), json!(displayed.ty.name()));
        if let Some(label) = &column.label {
            out.insert("label".into(), label_json(label));
        }
        for (key, on) in [
            ("required", column.required),
            ("unique", column.unique),
            ("hidden", column.hidden),
            ("title_field", column.title_field),
        ] {
            if on {
                out.insert(key.into(), json!(true));
            }
        }
        if let Some(default) = &column.default {
            out.insert("default".into(), default.clone());
        }
        if column.is_computed() {
            out.insert("computed".into(), json!(true));
        }
        if !column.is_stored() {
            out.insert("virtual".into(), json!(true));
        }
        if let Some(target) = &displayed.target {
            out.insert("target".into(), json!(target));
        }
        if let Some(values) = &displayed.values {
            out.insert("values".into(), json!(values));
        }
        JsonValue::Object(out)
    }

    // ------------------------------------------------------------- models.ts

    /// Types des enregistrements tels que l'API les envoie (noms de colonnes
    /// inchangés, décimaux et dates en texte), et un client typé par table.
    fn models(&self) -> String {
        let tables = self.model.tables();
        let example = tables
            .first()
            .map_or_else(String::new, |t| client_name(&t.name));
        let mut out = format!(
            "{HEADER}\n// Types des enregistrements, tels que l'API les envoie, pour le code\n\
             // personnalisé :\n//   const listing = await {example}(client).list();\n\n\
             import {{ type ForgeClient, type Json, TableClient }} from '@forge/web';\n"
        );
        for table in tables {
            out.push('\n');
            out += &self.interface(table);
        }
        out
    }

    fn interface(&self, table: &Table) -> String {
        let name = type_name(&table.name);
        let mut enums = String::new();
        let mut out = format!(
            "/** {} (table `{}`). */\nexport interface {name} {{\n  id: number;\n",
            self.label(table),
            table.name
        );
        for column in &table.columns {
            let (owner, displayed) = shown(self.model, &table.name, column);
            let ty = match displayed.ty {
                ColumnType::String | ColumnType::Text => "string".to_owned(),
                // Décimaux, dates et dates-heures : texte de l'API.
                ColumnType::Decimal | ColumnType::Date | ColumnType::Datetime => {
                    "string".to_owned()
                }
                ColumnType::Integer | ColumnType::Duration | ColumnType::Reference => {
                    "number".to_owned()
                }
                ColumnType::Boolean => "boolean".to_owned(),
                ColumnType::ReferenceList => "number[]".to_owned(),
                ColumnType::Enum => {
                    let enum_name = enum_name(owner, &displayed.name);
                    // Le type est déclaré avec la colonne qui porte les valeurs.
                    if owner == table.name && column.ty == ColumnType::Enum {
                        let values = displayed
                            .values
                            .iter()
                            .flatten()
                            .map(|v| format!("'{v}'"))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        let _ = writeln!(enums, "\nexport type {enum_name} = {values};");
                    }
                    enum_name
                }
                ColumnType::Lookup => "unknown".to_owned(),
            };
            let nullable = (!column.required || column.is_computed())
                && displayed.ty != ColumnType::ReferenceList;
            let null = if nullable { " | null" } else { "" };
            let _ = writeln!(out, "  {}: {ty}{null};", column.name);
        }
        out += "  owner: number | null;\n  created_at: string;\n  updated_at: string;\n}\n";
        let _ = write!(
            out,
            "{enums}\n/** Opérations typées sur la table `{table}`. */\n\
             export const {client} = (client: ForgeClient) =>\n  \
             new TableClient<{name}>(client, '{table}', (json: Json) => json as unknown as {name});\n",
            table = table.name,
            client = client_name(&table.name),
        );
        out
    }

    fn label(&self, table: &Table) -> String {
        table
            .label
            .as_ref()
            .and_then(|l| match l {
                Label::Plain(text) => Some(text.clone()),
                Label::Localized(map) => map.get(&self.model.spec().app.default_locale).cloned(),
            })
            .unwrap_or_else(|| table.name.clone())
    }
}

fn label_json(label: &Label) -> JsonValue {
    match label {
        Label::Plain(text) => json!(text),
        Label::Localized(map) => json!(map),
    }
}

/// Type d'un enregistrement : `PascalCase`, suffixe `Record` si homonyme d'un type global.
fn type_name(table: &str) -> String {
    let name = pascal_case(table);
    if GLOBAL_TYPES.contains(&name.as_str())
        || name == "TableClient"
        || name == "ForgeClient"
        || name == "Json"
    {
        name + "Record"
    } else {
        name
    }
}

fn enum_name(table: &str, column: &str) -> String {
    type_name(table) + &pascal_case(column)
}

/// Fabrique du client typé : `opportuniteApi`.
fn client_name(table: &str) -> String {
    camel_case(table) + "Api"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_names() {
        assert_eq!(type_name("opportunite"), "Opportunite");
        assert_eq!(type_name("date"), "DateRecord");
        assert_eq!(enum_name("opportunite", "etape"), "OpportuniteEtape");
        assert_eq!(client_name("date_cloture"), "dateClotureApi");
    }
}
