/// Description des tables d'une application, générée depuis `forge.json`
/// (`lib/generated/schema.dart`). Elle pilote toute l'interface : menus,
/// colonnes des listes, champs des formulaires, vues, droits affichés.
library;

/// Type d'une colonne. Un lookup prend le type de la colonne qu'il lit.
enum ColumnType {
  string,
  text,
  integer,
  decimal,
  boolean,
  date,
  datetime,

  /// Durée en secondes.
  duration,

  /// Valeur parmi [ColumnSchema.values] (`enum` dans `forge.json`).
  enumeration,

  /// Identifiant d'un enregistrement de [ColumnSchema.target].
  reference,

  /// Liste d'identifiants d'enregistrements de [ColumnSchema.target].
  referenceList;

  bool get isNumeric => this == integer || this == decimal || this == duration;

  bool get isTemporal => this == date || this == datetime;
}

/// Opération soumise aux règles d'autorisation.
enum Operation { read, create, update, delete }

/// Libellé : texte unique ou traductions par langue.
class Label {
  const Label(this.translations) : text = null;

  const Label.plain(String this.text) : translations = const {};

  final String? text;
  final Map<String, String> translations;

  /// Texte dans `locale`, sinon dans `fallbackLocale`, sinon la première traduction.
  String? resolve(String locale, String fallbackLocale) =>
      text ??
      translations[locale] ??
      translations[fallbackLocale] ??
      translations.values.firstOrNull;
}

class ColumnSchema {
  const ColumnSchema(
    this.name,
    this.type, {
    this.label,
    this.required = false,
    this.unique = false,
    this.hidden = false,
    this.titleField = false,
    this.defaultValue,
    this.computed = false,
    this.stored = true,
    this.target,
    this.values = const [],
  });

  final String name;
  final ColumnType type;
  final Label? label;
  final bool required;
  final bool unique;

  /// Absente de l'interface ; reste accessible par l'API.
  final bool hidden;

  /// Compose l'intitulé des enregistrements.
  final bool titleField;

  /// Valeur par défaut, au format JSON de l'API.
  final Object? defaultValue;

  /// Formule ou lookup : en lecture seule.
  final bool computed;

  /// Présente en base, donc utilisable pour trier et filtrer.
  final bool stored;

  /// Table visée par une référence.
  final String? target;

  /// Valeurs d'une énumération.
  final List<String> values;

  bool get writable => !computed;
}

/// Calendrier : début, et fin ou durée.
class CalendarView {
  const CalendarView({required this.start, this.end, this.duration});

  final String start;
  final String? end;
  final String? duration;
}

/// Statistiques : agrégats de colonnes numériques, regroupés ou non.
class StatsView {
  const StatsView({required this.fields, this.groupBy});

  final List<String> fields;
  final String? groupBy;
}

/// Règle d'autorisation. La condition éventuelle n'est connue que du serveur :
/// l'interface propose l'action, le serveur tranche.
class Rule {
  const Rule(this.roles, this.operations, {this.conditional = false});

  final List<String> roles;
  final Set<Operation> operations;
  final bool conditional;
}

/// Liste d'enregistrements d'une autre table qui référencent celui-ci.
class RelatedList {
  const RelatedList(this.table, this.column);

  final TableSchema table;

  /// Colonne `reference` de [table] qui vise l'enregistrement.
  final ColumnSchema column;
}

class TableSchema {
  const TableSchema(
    this.name, {
    this.label,
    required this.columns,
    this.calendar,
    this.stats,
    this.rules = const [],
  });

  final String name;
  final Label? label;
  final List<ColumnSchema> columns;
  final CalendarView? calendar;
  final StatsView? stats;
  final List<Rule> rules;

  ColumnSchema? column(String name) {
    for (final column in columns) {
      if (column.name == name) return column;
    }
    return null;
  }

  /// Colonnes affichées par l'interface.
  Iterable<ColumnSchema> get visibleColumns => columns.where((c) => !c.hidden);

  /// Colonnes saisies dans les formulaires.
  Iterable<ColumnSchema> get editableColumns =>
      visibleColumns.where((c) => c.writable);

  Iterable<ColumnSchema> get titleColumns => columns.where((c) => c.titleField);

  /// La recherche (`q`) porte sur les textes stockés.
  bool get searchable => columns.any(
    (c) =>
        c.stored && (c.type == ColumnType.string || c.type == ColumnType.text),
  );

  /// Un rôle de l'utilisateur peut tenter `operation` (`admin` peut tout).
  bool allows(Iterable<String> roles, Operation operation) =>
      roles.contains(adminRole) ||
      rules.any(
        (r) => r.operations.contains(operation) && r.roles.any(roles.contains),
      );
}

class Parameter {
  const Parameter(this.name, this.type, {this.label});

  final String name;
  final ColumnType type;
  final Label? label;
}

/// Rôle qui a tous les droits.
const adminRole = 'admin';

class AppSchema {
  const AppSchema({
    required this.name,
    required this.defaultLocale,
    required this.locales,
    required this.roles,
    this.parameters = const [],
    required this.tables,
  });

  final String name;
  final String defaultLocale;
  final List<String> locales;
  final List<String> roles;
  final List<Parameter> parameters;
  final List<TableSchema> tables;

  TableSchema? table(String name) {
    for (final table in tables) {
      if (table.name == name) return table;
    }
    return null;
  }

  /// Listes d'enregistrements qui référencent un enregistrement de `table`.
  List<RelatedList> relatedLists(TableSchema table) => [
    for (final other in tables)
      for (final column in other.visibleColumns)
        if (column.type == ColumnType.reference &&
            column.stored &&
            !column.computed &&
            column.target == table.name)
          RelatedList(other, column),
  ];
}
