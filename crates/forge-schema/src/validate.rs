//! Validation sémantique d'un [`Spec`] et construction du [`Model`].
//!
//! Toutes les erreurs sont collectées (pas d'arrêt au premier problème) et
//! localisées par leur chemin JSON. Les erreurs dérivées d'une erreur déjà
//! signalée (ex. colonne d'une table cible inconnue) ne sont pas répétées.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use forge_formula::{Expr, ExprKind, FunctionKind, FunctionRegistry, VarScope};

use crate::error::{Issue, SchemaError};
use crate::graph;
use crate::model::{ColumnRef, Model, Relation, RelationKind};
use crate::names::{SYSTEM_COLUMNS, SYSTEM_TABLES, USER_FIELDS, check_identifier, check_locale};
use crate::spec::{Action, Column, ColumnType, Label, Spec, Table};

pub(crate) fn validate(spec: Spec) -> Result<Model, SchemaError> {
    let registry = FunctionRegistry::builtin();
    let mut validator = Validator::new(&spec, &registry);
    validator.run();

    let Validator {
        issues,
        relations,
        formulas,
        lookups,
        conditions,
        computed_order,
        ..
    } = validator;
    if !issues.is_empty() {
        return Err(SchemaError { issues });
    }
    Ok(Model {
        spec,
        relations,
        formulas,
        lookups,
        conditions,
        computed_order,
    })
}

/// Une colonne vue par la résolution de chemins : métier ou système.
#[derive(Clone, Copy)]
enum Field<'a> {
    System(ColumnType),
    User(&'a Column),
}

impl<'a> Field<'a> {
    fn system(name: &str) -> Option<Self> {
        let ty = match name {
            "id" | "owner" => ColumnType::Integer,
            "created_at" | "updated_at" => ColumnType::Datetime,
            _ => return None,
        };
        Some(Self::System(ty))
    }

    fn ty(self) -> ColumnType {
        match self {
            Self::System(ty) => ty,
            Self::User(column) => column.ty,
        }
    }

    fn column(self) -> Option<&'a Column> {
        match self {
            Self::System(_) => None,
            Self::User(column) => Some(column),
        }
    }
}

/// Aboutissement d'un chemin résolu.
enum End {
    Column(ColumnRef),
    /// Le chemin désigne une relation « plusieurs » (ex. `COUNT(tags)`).
    Relation,
}

struct Resolved {
    end: End,
    /// Le chemin traverse une relation « plusieurs ».
    many: bool,
}

/// Usage d'une expression, qui détermine ce qu'elle a le droit de référencer.
#[derive(Clone, Copy)]
enum Usage {
    Formula {
        persist: bool,
    },
    /// Condition `when` d'une règle : doit rester traduisible en SQL.
    Condition,
}

struct Validator<'a> {
    spec: &'a Spec,
    registry: &'a FunctionRegistry,
    tables: HashMap<&'a str, (usize, &'a Table)>,
    /// Relations inverses : (table cible, nom) → table source.
    inverses: HashMap<(&'a str, &'a str), &'a str>,
    issues: Vec<Issue>,
    relations: Vec<Relation>,
    formulas: BTreeMap<ColumnRef, Expr>,
    lookups: BTreeMap<ColumnRef, Vec<String>>,
    conditions: BTreeMap<(String, usize), Expr>,
    dependencies: BTreeMap<ColumnRef, BTreeSet<ColumnRef>>,
    computed_order: Vec<ColumnRef>,
}

impl<'a> Validator<'a> {
    fn new(spec: &'a Spec, registry: &'a FunctionRegistry) -> Self {
        Self {
            spec,
            registry,
            tables: HashMap::new(),
            inverses: HashMap::new(),
            issues: Vec::new(),
            relations: Vec::new(),
            formulas: BTreeMap::new(),
            lookups: BTreeMap::new(),
            conditions: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            computed_order: Vec::new(),
        }
    }

    fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue::new(path, message));
    }

    fn run(&mut self) {
        self.check_app();
        self.check_roles();
        self.check_parameters();
        self.check_tables();
        self.build_relations();
        self.check_required_references();
        self.check_expressions();
        self.check_views();
        self.check_rules();
        self.check_cycles();
    }

    // ---------------------------------------------------------------- app

    fn check_app(&mut self) {
        let app = &self.spec.app;
        if let Err(msg) = check_identifier(&app.name) {
            self.error("app.name", msg);
        }
        if app.locales.is_empty() {
            self.error("app.locales", "au moins une langue est requise");
        }
        let mut seen = HashSet::new();
        for (i, locale) in app.locales.iter().enumerate() {
            if let Err(msg) = check_locale(locale) {
                self.error(format!("app.locales[{i}]"), msg);
            } else if !seen.insert(locale) {
                self.error(
                    format!("app.locales[{i}]"),
                    format!("langue `{locale}` en double"),
                );
            }
        }
        if !app.locales.contains(&app.default_locale) {
            self.error(
                "app.default_locale",
                format!("`{}` doit figurer dans `app.locales`", app.default_locale),
            );
        }
    }

    fn check_label(&mut self, path: &str, label: Option<&Label>) {
        let Some(Label::Localized(translations)) = label else {
            return;
        };
        for locale in translations.keys() {
            if !self.spec.app.locales.contains(locale) {
                self.error(
                    format!("{path}.label.{locale}"),
                    format!("langue `{locale}` absente de `app.locales`"),
                );
            }
        }
    }

    // -------------------------------------------------------------- roles

    fn check_roles(&mut self) {
        let mut seen = HashSet::new();
        for (i, role) in self.spec.roles.iter().enumerate() {
            if let Err(msg) = check_identifier(role) {
                self.error(format!("roles[{i}]"), msg);
            } else if !seen.insert(role) {
                self.error(format!("roles[{i}]"), format!("rôle `{role}` en double"));
            }
        }
        if !self.spec.roles.iter().any(|r| r == "admin") {
            self.error(
                "roles",
                "le rôle `admin` est obligatoire (compte administrateur initial)",
            );
        }
    }

    // --------------------------------------------------------- parameters

    fn check_parameters(&mut self) {
        let mut seen = HashSet::new();
        for (i, param) in self.spec.parameters.iter().enumerate() {
            let path = format!("parameters[{i}]");
            if let Err(msg) = check_identifier(&param.name) {
                self.error(format!("{path}.name"), msg);
            } else if !seen.insert(&param.name) {
                self.error(
                    format!("{path}.name"),
                    format!("paramètre `{}` en double", param.name),
                );
            }
            if !param.ty.is_scalar() {
                self.error(
                    format!("{path}.type"),
                    format!("type `{}` non autorisé pour un paramètre", param.ty.name()),
                );
            } else if let Some(default) = &param.default {
                if let Err(msg) = check_value(param.ty, None, default) {
                    self.error(format!("{path}.default"), msg);
                }
            }
            self.check_label(&path, param.label.as_ref());
        }
    }

    // ------------------------------------------------------------- tables

    fn check_tables(&mut self) {
        if self.spec.tables.is_empty() {
            self.error("tables", "au moins une table est requise");
        }
        for (i, table) in self.spec.tables.iter().enumerate() {
            let path = format!("tables[{i}]");
            if let Err(msg) = check_identifier(&table.name) {
                self.error(format!("{path}.name"), msg);
            } else if SYSTEM_TABLES.contains(&table.name.as_str()) {
                self.error(
                    format!("{path}.name"),
                    format!("`{}` est une table système de forge", table.name),
                );
            } else if self.tables.contains_key(table.name.as_str()) {
                self.error(
                    format!("{path}.name"),
                    format!("table `{}` en double", table.name),
                );
            } else {
                self.tables.insert(&table.name, (i, table));
            }
            self.check_label(&path, table.label.as_ref());

            if table.columns.is_empty() {
                self.error(
                    format!("{path}.columns"),
                    "au moins une colonne est requise",
                );
            }
            let mut seen = HashSet::new();
            for (j, column) in table.columns.iter().enumerate() {
                let path = format!("{path}.columns[{j}]");
                if !seen.insert(&column.name) {
                    self.error(
                        format!("{path}.name"),
                        format!("colonne `{}` en double", column.name),
                    );
                }
                self.check_column(&path, column);
                if let Some(old) = &column.renamed_from {
                    let taken = table.columns.iter().any(|c| c.name == *old)
                        || table
                            .columns
                            .iter()
                            .filter(|c| c.renamed_from.as_ref() == Some(old))
                            .count()
                            > 1;
                    if taken {
                        self.error(
                            format!("{path}.renamed_from"),
                            format!("`{old}` est encore une colonne de la table, ou l'ancien nom d'une autre"),
                        );
                    }
                }
            }
        }
    }

    fn check_column(&mut self, path: &str, column: &Column) {
        let ty = column.ty;
        if let Err(msg) = check_identifier(&column.name) {
            self.error(format!("{path}.name"), msg);
        } else if SYSTEM_COLUMNS.contains(&column.name.as_str()) {
            self.error(
                format!("{path}.name"),
                format!("`{}` est une colonne ajoutée automatiquement", column.name),
            );
        }
        if let Some(old) = &column.renamed_from {
            if let Err(msg) = check_identifier(old) {
                self.error(format!("{path}.renamed_from"), msg);
            }
        }
        self.check_label(path, column.label.as_ref());

        // Options propres à certains types.
        let relational = matches!(ty, ColumnType::Reference | ColumnType::ReferenceList);
        self.expect_option(path, "target", column.target.is_some(), relational, ty);
        self.expect_option(
            path,
            "values",
            column.values.is_some(),
            ty == ColumnType::Enum,
            ty,
        );
        self.expect_option(
            path,
            "path",
            column.path.is_some(),
            ty == ColumnType::Lookup,
            ty,
        );
        if column.inverse.is_some() && !relational {
            self.error(
                format!("{path}.inverse"),
                "réservé aux colonnes `reference` et `reference_list`",
            );
        }
        if let Some(values) = &column.values {
            self.check_enum_values(path, values);
        }

        // Colonnes calculées.
        if column.formula.is_some() && !ty.is_scalar() {
            self.error(
                format!("{path}.formula"),
                format!("une colonne `{}` ne peut pas être calculée", ty.name()),
            );
        }
        if column.persist && column.formula.is_none() {
            self.error(
                format!("{path}.persist"),
                "`persist` nécessite une `formula`",
            );
        }
        if column.is_computed() {
            if column.required {
                self.error(
                    format!("{path}.required"),
                    "une colonne calculée ne peut pas être obligatoire",
                );
            }
            if column.default.is_some() {
                self.error(
                    format!("{path}.default"),
                    "une colonne calculée n'a pas de valeur par défaut",
                );
            }
        }

        if column.unique && (!column.is_stored() || ty == ColumnType::Text) {
            self.error(
                format!("{path}.unique"),
                "`unique` exige une colonne stockée en base et indexable (pas `text`, \
                 `reference_list`, `lookup` ni formule non persistée)",
            );
        }
        if column.title_field && (column.hidden || ty == ColumnType::ReferenceList) {
            self.error(
                format!("{path}.title_field"),
                "un champ d'intitulé ne peut être ni `hidden` ni `reference_list`",
            );
        }
        if let Some(default) = &column.default {
            if matches!(ty, ColumnType::Reference | ColumnType::ReferenceList) {
                self.error(
                    format!("{path}.default"),
                    "pas de valeur par défaut pour une relation",
                );
            } else if !column.is_computed() {
                if let Err(msg) = check_value(ty, column.values.as_deref(), default) {
                    self.error(format!("{path}.default"), msg);
                }
            }
        }
    }

    fn expect_option(
        &mut self,
        path: &str,
        option: &str,
        present: bool,
        expected: bool,
        ty: ColumnType,
    ) {
        match (present, expected) {
            (true, false) => self.error(
                format!("{path}.{option}"),
                format!("option inutile pour une colonne `{}`", ty.name()),
            ),
            (false, true) => self.error(
                path.to_owned(),
                format!("une colonne `{}` exige l'option `{option}`", ty.name()),
            ),
            _ => {}
        }
    }

    fn check_enum_values(&mut self, path: &str, values: &[String]) {
        if values.is_empty() {
            self.error(format!("{path}.values"), "au moins une valeur est requise");
        }
        let mut seen = HashSet::new();
        for (k, value) in values.iter().enumerate() {
            if let Err(msg) = check_identifier(value) {
                self.error(format!("{path}.values[{k}]"), msg);
            } else if !seen.insert(value) {
                self.error(
                    format!("{path}.values[{k}]"),
                    format!("valeur `{value}` en double"),
                );
            }
        }
    }

    // ---------------------------------------------------------- relations

    fn build_relations(&mut self) {
        for (i, table) in self.spec.tables.iter().enumerate() {
            for (j, column) in table.columns.iter().enumerate() {
                let kind = match column.ty {
                    ColumnType::Reference => RelationKind::ManyToOne,
                    ColumnType::ReferenceList => RelationKind::ManyToMany,
                    _ => continue,
                };
                let Some(target) = column.target.as_deref() else {
                    continue; // déjà signalé
                };
                let path = format!("tables[{i}].columns[{j}]");
                let Some((_, target_table)) = self.tables.get(target).copied() else {
                    self.error(
                        format!("{path}.target"),
                        format!("table `{target}` inconnue"),
                    );
                    continue;
                };

                let inverse = column.inverse.as_deref().unwrap_or(&table.name);
                let inverse_path = if column.inverse.is_some() {
                    format!("{path}.inverse")
                } else {
                    path.clone()
                };
                if let Err(msg) = check_identifier(inverse) {
                    self.error(inverse_path, msg);
                    continue;
                }
                let conflict = if target_table.columns.iter().any(|c| c.name == inverse)
                    || SYSTEM_COLUMNS.contains(&inverse)
                {
                    Some(format!(
                        "la table `{target}` a déjà une colonne `{inverse}`"
                    ))
                } else if self.inverses.contains_key(&(target, inverse)) {
                    Some(format!(
                        "la table `{target}` a déjà une relation inverse `{inverse}`"
                    ))
                } else {
                    None
                };
                if let Some(conflict) = conflict {
                    self.error(
                        inverse_path,
                        format!("{conflict} : choisissez un autre nom avec l'option `inverse`"),
                    );
                    continue;
                }

                let relation = Relation {
                    source: ColumnRef::new(&table.name, &column.name),
                    target: target.to_owned(),
                    kind,
                    inverse: inverse.to_owned(),
                };
                if let Some(join) = relation.join_table() {
                    let taken = self.tables.contains_key(join.as_str())
                        || SYSTEM_TABLES.contains(&join.as_str())
                        || self
                            .relations
                            .iter()
                            .any(|r| r.join_table().as_ref() == Some(&join));
                    if taken {
                        self.error(
                            path,
                            format!("la table de jointure `{join}` entre en conflit avec une autre table"),
                        );
                        continue;
                    }
                }
                self.inverses.insert((target, inverse), &table.name);
                self.relations.push(relation);
            }
        }
    }

    /// Un cycle de références obligatoires empêcherait toute insertion.
    fn check_required_references(&mut self) {
        let spec = self.spec;
        let mut edges: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for table in &spec.tables {
            let deps = edges.entry(table.name.as_str()).or_default();
            for column in table
                .columns
                .iter()
                .filter(|c| c.required && c.ty == ColumnType::Reference)
            {
                if let Some(target) = column
                    .target
                    .as_deref()
                    .filter(|t| self.tables.contains_key(t))
                {
                    deps.insert(target);
                }
            }
        }
        let cycles = graph::sort(&edges).cycles;
        for cycle in cycles {
            let first = cycle[0];
            let path = self
                .tables
                .get(first)
                .map(|(i, _)| format!("tables[{i}]"))
                .unwrap_or_default();
            self.error(
                path,
                format!(
                    "références obligatoires circulaires ({}) : aucun enregistrement ne pourrait être créé ; \
                     rendez l'une d'elles facultative",
                    cycle.join(" → ")
                ),
            );
        }
    }

    // ---------------------------------------------------------- résolution

    fn field(&self, table: &str, name: &str) -> Option<Field<'a>> {
        let (_, table) = self.tables.get(table)?;
        table
            .columns
            .iter()
            .find(|c| c.name == name)
            .map(Field::User)
            .or_else(|| Field::system(name))
    }

    /// Résout un chemin depuis `table`.
    ///
    /// `Err(None)` signale un échec dû à une erreur déjà rapportée ailleurs.
    fn resolve(&self, table: &'a str, path: &[String]) -> Result<Resolved, Option<String>> {
        let mut current = table;
        let mut many = false;
        for (i, segment) in path.iter().enumerate() {
            let last = i + 1 == path.len();
            if let Some(field) = self.field(current, segment) {
                let target = field.column().and_then(|c| c.target.as_deref());
                match field.ty() {
                    ColumnType::Reference | ColumnType::ReferenceList => {
                        let target = target.filter(|t| self.tables.contains_key(t)).ok_or(None)?;
                        if field.ty() == ColumnType::ReferenceList {
                            many = true;
                            if last {
                                return Ok(Resolved {
                                    end: End::Relation,
                                    many,
                                });
                            }
                        } else if last {
                            let owner = ColumnRef::new(current, segment.as_str());
                            return Ok(Resolved {
                                end: End::Column(owner),
                                many,
                            });
                        }
                        current = self.tables[target].1.name.as_str();
                    }
                    _ if last => {
                        let owner = ColumnRef::new(current, segment.as_str());
                        return Ok(Resolved {
                            end: End::Column(owner),
                            many,
                        });
                    }
                    _ => {
                        return Err(Some(format!(
                            "`{segment}` n'est pas une référence : impossible d'accéder à `.{}`",
                            path[i + 1]
                        )));
                    }
                }
            } else if let Some(source) = self.inverses.get(&(current, segment.as_str())) {
                many = true;
                if last {
                    return Ok(Resolved {
                        end: End::Relation,
                        many,
                    });
                }
                current = source;
            } else {
                return Err(Some(format!(
                    "`{segment}` n'est ni une colonne ni une relation de la table `{current}`"
                )));
            }
        }
        Err(Some("chemin vide".to_owned()))
    }

    // --------------------------------------------------------- expressions

    fn check_expressions(&mut self) {
        let spec = self.spec;
        for (i, table) in spec.tables.iter().enumerate() {
            if self
                .tables
                .get(table.name.as_str())
                .is_none_or(|(k, _)| *k != i)
            {
                continue; // nom invalide ou en double, déjà signalé
            }
            for (j, column) in table.columns.iter().enumerate() {
                let owner = ColumnRef::new(&table.name, &column.name);
                if let Some(src) = &column.formula {
                    let path = format!("tables[{i}].columns[{j}].formula");
                    let usage = Usage::Formula {
                        persist: column.persist,
                    };
                    if let Some((expr, deps)) = self.analyze(&path, &table.name, src, usage) {
                        self.formulas.insert(owner.clone(), expr);
                        self.dependencies.insert(owner.clone(), deps);
                    }
                }
                if let Some(src) = &column.path {
                    let path = format!("tables[{i}].columns[{j}].path");
                    if let Some((segments, dep)) = self.check_lookup(&path, &table.name, src) {
                        self.lookups.insert(owner.clone(), segments);
                        self.dependencies.insert(owner, BTreeSet::from([dep]));
                    }
                }
            }
        }
    }

    /// Analyse une expression ; retourne son AST et les colonnes dont elle dépend.
    fn analyze(
        &mut self,
        path: &str,
        table: &'a str,
        src: &str,
        usage: Usage,
    ) -> Option<(Expr, BTreeSet<ColumnRef>)> {
        let expr = match forge_formula::parse(src) {
            Ok(expr) => expr,
            Err(err) => {
                self.error(path, format!("syntaxe : {err}"));
                return None;
            }
        };
        let mut check = ExprCheck {
            validator: &*self,
            table,
            usage,
            deps: BTreeSet::new(),
            errors: Vec::new(),
        };
        check.expr(&expr, false);
        let ExprCheck { deps, errors, .. } = check;
        let valid = errors.is_empty();
        for error in errors {
            self.error(path, error);
        }
        valid.then_some((expr, deps))
    }

    fn check_lookup(
        &mut self,
        path: &str,
        table: &'a str,
        src: &str,
    ) -> Option<(Vec<String>, ColumnRef)> {
        let segments = match forge_formula::parse(src).map(|e| e.kind) {
            Ok(ExprKind::Path(segments)) if segments.len() >= 2 => segments,
            _ => {
                self.error(path, "chemin attendu, de la forme `reference.colonne`");
                return None;
            }
        };
        match self.resolve(table, &segments) {
            Ok(Resolved {
                end: End::Column(owner),
                many: false,
            }) => Some((segments, owner)),
            Ok(_) => {
                self.error(
                    path,
                    "un lookup doit mener à une seule valeur (pas de relation « plusieurs »)",
                );
                None
            }
            Err(Some(msg)) => {
                self.error(path, msg);
                None
            }
            Err(None) => None,
        }
    }

    /// Type effectif d'une colonne ; pour un lookup, celui de la colonne visée.
    fn effective_type(&self, column: &ColumnRef) -> Option<ColumnType> {
        let mut current = column.clone();
        for _ in 0..=self.lookups.len() {
            let ty = self.field(&current.table, &current.column)?.ty();
            if ty != ColumnType::Lookup {
                return Some(ty);
            }
            current = self.dependencies.get(&current)?.first()?.clone();
        }
        None // cycle de lookups, signalé par check_cycles
    }

    // --------------------------------------------------------------- vues

    fn check_views(&mut self) {
        let spec = self.spec;
        for (i, table) in spec.tables.iter().enumerate() {
            let path = format!("tables[{i}].views");
            if let Some(calendar) = &table.views.calendar {
                let path = format!("{path}.calendar");
                let start = self.view_column(
                    &format!("{path}.start"),
                    table,
                    &calendar.start,
                    "date ou datetime",
                    ColumnType::is_temporal,
                );
                if let Some(end) = &calendar.end {
                    let end_ty = self.view_column(
                        &format!("{path}.end"),
                        table,
                        end,
                        "date ou datetime",
                        ColumnType::is_temporal,
                    );
                    if let (Some(start), Some(end_ty)) = (start, end_ty) {
                        if start != end_ty {
                            self.error(
                                format!("{path}.end"),
                                "`end` doit avoir le même type que `start`",
                            );
                        }
                    }
                }
                if let Some(duration) = &calendar.duration {
                    self.view_column(
                        &format!("{path}.duration"),
                        table,
                        duration,
                        "duration",
                        |ty| ty == ColumnType::Duration,
                    );
                }
                if calendar.end.is_some() && calendar.duration.is_some() {
                    self.error(path, "`end` et `duration` sont exclusifs");
                }
            }
            if let Some(stats) = &table.views.stats {
                let path = format!("{path}.stats");
                if stats.fields.is_empty() {
                    self.error(format!("{path}.fields"), "au moins une colonne est requise");
                }
                for (k, field) in stats.fields.iter().enumerate() {
                    self.view_column(
                        &format!("{path}.fields[{k}]"),
                        table,
                        field,
                        "numérique",
                        ColumnType::is_numeric,
                    );
                }
                if let Some(group_by) = &stats.group_by {
                    self.view_column(
                        &format!("{path}.group_by"),
                        table,
                        group_by,
                        "enum, boolean, reference ou string",
                        |ty| {
                            matches!(
                                ty,
                                ColumnType::Enum
                                    | ColumnType::Boolean
                                    | ColumnType::Reference
                                    | ColumnType::String
                            )
                        },
                    );
                }
            }
        }
    }

    /// Vérifie qu'une vue désigne une colonne du type attendu ; retourne son type.
    fn view_column(
        &mut self,
        path: &str,
        table: &Table,
        name: &str,
        expected: &str,
        accepts: impl Fn(ColumnType) -> bool,
    ) -> Option<ColumnType> {
        if self.field(&table.name, name).is_none() {
            self.error(
                path,
                format!("colonne `{name}` inconnue dans `{}`", table.name),
            );
            return None;
        }
        let ty = self.effective_type(&ColumnRef::new(&table.name, name))?;
        if accepts(ty) {
            Some(ty)
        } else {
            self.error(
                path,
                format!("`{name}` est de type `{}`, attendu : {expected}", ty.name()),
            );
            None
        }
    }

    // ------------------------------------------------------------- règles

    fn check_rules(&mut self) {
        let spec = self.spec;
        for (i, table) in spec.tables.iter().enumerate() {
            let known_table = self
                .tables
                .get(table.name.as_str())
                .is_some_and(|(k, _)| *k == i);
            for (r, rule) in table.rules.iter().enumerate() {
                let path = format!("tables[{i}].rules[{r}]");
                if rule.roles.is_empty() {
                    self.error(format!("{path}.roles"), "au moins un rôle est requis");
                }
                for (k, role) in rule.roles.iter().enumerate() {
                    if !spec.roles.contains(role) {
                        self.error(
                            format!("{path}.roles[{k}]"),
                            format!("rôle `{role}` non déclaré dans `roles`"),
                        );
                    }
                }
                if rule.actions.is_empty() {
                    self.error(format!("{path}.actions"), "au moins une action est requise");
                }
                if rule.actions.contains(&Action::All) && rule.actions.len() > 1 {
                    self.error(
                        format!("{path}.actions"),
                        "`*` couvre déjà toutes les actions",
                    );
                }
                if let (Some(src), true) = (&rule.when, known_table) {
                    if let Some((expr, _)) =
                        self.analyze(&format!("{path}.when"), &table.name, src, Usage::Condition)
                    {
                        self.conditions.insert((table.name.clone(), r), expr);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------- cycles

    fn check_cycles(&mut self) {
        let sorted = graph::sort(&self.dependencies);
        for cycle in sorted.cycles {
            let first = &cycle[0];
            let chain: Vec<String> = cycle.iter().map(ToString::to_string).collect();
            let path = self.column_path(first);
            self.error(
                path,
                format!("dépendance circulaire : {}", chain.join(" → ")),
            );
        }
        self.computed_order = sorted.order;
    }

    fn column_path(&self, column: &ColumnRef) -> String {
        let Some((i, table)) = self.tables.get(column.table.as_str()) else {
            return String::new();
        };
        let j = table
            .columns
            .iter()
            .position(|c| c.name == column.column)
            .unwrap_or_default();
        let option = if table.columns[j].ty == ColumnType::Lookup {
            "path"
        } else {
            "formula"
        };
        format!("tables[{i}].columns[{j}].{option}")
    }
}

/// Parcours d'une expression dans le contexte d'une table.
struct ExprCheck<'v, 'a> {
    validator: &'v Validator<'a>,
    table: &'a str,
    usage: Usage,
    deps: BTreeSet<ColumnRef>,
    errors: Vec<String>,
}

impl ExprCheck<'_, '_> {
    fn error(&mut self, expr: &Expr, message: impl std::fmt::Display) {
        self.errors
            .push(format!("{message} (position {})", expr.span.start));
    }

    fn expr(&mut self, expr: &Expr, in_aggregate: bool) {
        match &expr.kind {
            ExprKind::Number(_) | ExprKind::String(_) | ExprKind::Bool(_) | ExprKind::Null => {}
            ExprKind::Path(path) => self.path(expr, path, in_aggregate),
            ExprKind::Var { scope, path } => self.var(expr, *scope, path),
            ExprKind::Unary { expr: inner, .. } => self.expr(inner, in_aggregate),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left, in_aggregate);
                self.expr(right, in_aggregate);
            }
            ExprKind::Call { name, args } => self.call(expr, name, args, in_aggregate),
        }
    }

    fn path(&mut self, expr: &Expr, path: &[String], in_aggregate: bool) {
        let joined = path.join(".");
        if let Usage::Condition = self.usage {
            let stored = path.len() == 1
                && self.validator.field(self.table, &path[0]).is_some_and(|f| {
                    f.column().is_none_or(Column::is_stored) && f.ty() != ColumnType::ReferenceList
                });
            if !stored {
                self.error(
                    expr,
                    format!("`{joined}` : une condition ne peut utiliser que les colonnes stockées de la table"),
                );
            }
            return;
        }
        match self.validator.resolve(self.table, path) {
            Ok(Resolved { many: true, .. }) if !in_aggregate => self.error(
                expr,
                format!("`{joined}` désigne plusieurs valeurs : utilisez un agrégat (SUM, AVG, MIN, MAX, COUNT)"),
            ),
            Ok(Resolved { end: End::Column(owner), .. }) => {
                self.deps.insert(owner);
            }
            Ok(Resolved { end: End::Relation, .. }) | Err(None) => {}
            Err(Some(msg)) => self.error(expr, msg),
        }
    }

    fn var(&mut self, expr: &Expr, scope: VarScope, path: &[String]) {
        let joined = path.join(".");
        match scope {
            VarScope::User if matches!(self.usage, Usage::Formula { .. }) => {
                self.error(
                    expr,
                    "`$user` n'est utilisable que dans les conditions de règles",
                );
            }
            VarScope::User if path.len() != 1 || !USER_FIELDS.contains(&path[0].as_str()) => {
                self.error(
                    expr,
                    format!(
                        "`$user.{joined}` inconnu (champs disponibles : {})",
                        USER_FIELDS.join(", ")
                    ),
                );
            }
            VarScope::Param
                if path.len() != 1
                    || !self
                        .validator
                        .spec
                        .parameters
                        .iter()
                        .any(|p| p.name == path[0]) =>
            {
                self.error(
                    expr,
                    format!("paramètre `$param.{joined}` non déclaré dans `parameters`"),
                );
            }
            VarScope::User | VarScope::Param => {}
        }
    }

    fn call(&mut self, expr: &Expr, name: &str, args: &[Expr], in_aggregate: bool) {
        if let Usage::Condition = self.usage {
            self.error(
                expr,
                format!("`{name}` : les fonctions ne sont pas autorisées dans une condition"),
            );
            return;
        }
        let Some(signature) = self.validator.registry.get(name) else {
            let known: Vec<_> = self.validator.registry.names().collect();
            self.error(
                expr,
                format!(
                    "fonction `{name}` inconnue (disponibles : {})",
                    known.join(", ")
                ),
            );
            return;
        };
        if !signature.arity.accepts(args.len()) {
            self.error(
                expr,
                format!(
                    "`{name}` attend {} argument(s), {} fourni(s)",
                    signature.arity,
                    args.len()
                ),
            );
        }
        if signature.volatile && matches!(self.usage, Usage::Formula { persist: true }) {
            self.error(
                expr,
                format!("`{name}` change avec le temps : interdit dans une formule persistée"),
            );
        }
        match signature.kind {
            FunctionKind::Scalar => {
                for arg in args {
                    self.expr(arg, in_aggregate);
                }
            }
            FunctionKind::Aggregate if in_aggregate => {
                self.error(expr, format!("`{name}` : agrégats imbriqués non supportés"));
            }
            FunctionKind::Aggregate => {
                for arg in args {
                    self.aggregate_arg(name, arg);
                }
            }
        }
    }

    fn aggregate_arg(&mut self, function: &str, arg: &Expr) {
        let ExprKind::Path(path) = &arg.kind else {
            self.error(
                arg,
                format!(
                    "`{function}` attend une relation, par exemple `{function}(lignes.montant)`"
                ),
            );
            return;
        };
        match self.validator.resolve(self.table, path) {
            Ok(Resolved { many: false, .. }) => self.error(
                arg,
                format!(
                    "`{}` n'est pas une relation « plusieurs » : `{function}` est inutile",
                    path.join(".")
                ),
            ),
            Ok(Resolved {
                end: End::Relation, ..
            }) if function != "COUNT" => self.error(
                arg,
                format!(
                    "`{function}` attend une colonne de la relation, par exemple `{}.montant`",
                    path.join(".")
                ),
            ),
            Ok(Resolved {
                end: End::Column(owner),
                ..
            }) => {
                self.deps.insert(owner);
            }
            Ok(Resolved {
                end: End::Relation, ..
            })
            | Err(None) => {}
            Err(Some(msg)) => self.error(arg, msg),
        }
    }
}

/// Vérifie qu'une valeur JSON (défaut d'une colonne ou d'un paramètre) correspond au type.
fn check_value(
    ty: ColumnType,
    values: Option<&[String]>,
    value: &serde_json::Value,
) -> Result<(), String> {
    crate::value::from_json(ty, values, value).map(drop)
}
