/**
 * Description des tables d'une application, générée depuis `forge.json`
 * (`src/generated/schema.ts`). Elle pilote toute l'interface : menus,
 * colonnes des listes, champs des formulaires, vues, droits affichés.
 */

/** Type d'une colonne ; un lookup prend le type de la colonne qu'il lit. */
export type ColumnType =
  | 'string'
  | 'text'
  | 'integer'
  | 'decimal'
  | 'boolean'
  | 'date'
  | 'datetime'
  /** Durée en secondes. */
  | 'duration'
  | 'enum'
  | 'reference'
  | 'reference_list';

/** Opération soumise aux règles d'autorisation. */
export type Operation = 'read' | 'create' | 'update' | 'delete';

/** Libellé : texte unique ou traductions par langue. */
export type Label = string | Record<string, string>;

export interface ColumnSchema {
  name: string;
  type: ColumnType;
  label?: Label;
  required?: boolean;
  unique?: boolean;
  /** Absente de l'interface ; reste accessible par l'API. */
  hidden?: boolean;
  /** Compose l'intitulé des enregistrements. */
  title_field?: boolean;
  /** Valeur par défaut, au format JSON de l'API. */
  default?: unknown;
  /** Formule ou lookup : en lecture seule. */
  computed?: boolean;
  /** Absente de la base (formule non persistée, lookup) : ni tri ni filtre. */
  virtual?: boolean;
  /** Table visée par une référence. */
  target?: string;
  /** Valeurs d'une énumération. */
  values?: string[];
}

export interface CalendarView {
  start: string;
  end?: string;
  duration?: string;
}

export interface StatsView {
  fields: string[];
  group_by?: string;
}

/** Règle d'autorisation ; une condition n'est connue que du serveur. */
export interface Rule {
  roles: string[];
  operations: Operation[];
  conditional?: boolean;
}

export interface TableSchema {
  name: string;
  label?: Label;
  columns: ColumnSchema[];
  calendar?: CalendarView;
  stats?: StatsView;
  rules?: Rule[];
}

export interface Parameter {
  name: string;
  type: ColumnType;
  label?: Label;
}

export interface AppSchema {
  name: string;
  default_locale: string;
  locales: string[];
  roles: string[];
  parameters?: Parameter[];
  tables: TableSchema[];
}

/** Rôle qui a tous les droits. */
export const ADMIN_ROLE = 'admin';

export const isNumeric = (type: ColumnType) =>
  type === 'integer' || type === 'decimal' || type === 'duration';

export const findTable = (schema: AppSchema, name: string) =>
  schema.tables.find((t) => t.name === name);

export const findColumn = (table: TableSchema, name: string) =>
  table.columns.find((c) => c.name === name);

/** Colonnes affichées par l'interface. */
export const visibleColumns = (table: TableSchema) =>
  table.columns.filter((c) => !c.hidden);

/** Colonnes saisies dans les formulaires. */
export const editableColumns = (table: TableSchema) =>
  visibleColumns(table).filter((c) => !c.computed);

export const titleColumns = (table: TableSchema) =>
  table.columns.filter((c) => c.title_field);

/** La recherche (`q`) porte sur les textes stockés. */
export const isSearchable = (table: TableSchema) =>
  table.columns.some(
    (c) => !c.virtual && (c.type === 'string' || c.type === 'text'),
  );

/** Un rôle de l'utilisateur peut tenter `operation` (`admin` peut tout). */
export const allows = (
  table: TableSchema,
  roles: readonly string[],
  operation: Operation,
) =>
  roles.includes(ADMIN_ROLE) ||
  (table.rules ?? []).some(
    (r) => r.operations.includes(operation) && r.roles.some((role) => roles.includes(role)),
  );

/** Liste d'enregistrements d'une autre table qui référencent celui-ci. */
export interface RelatedList {
  table: TableSchema;
  column: ColumnSchema;
}

export const relatedLists = (
  schema: AppSchema,
  table: TableSchema,
): RelatedList[] =>
  schema.tables.flatMap((other) =>
    visibleColumns(other)
      .filter(
        (c) =>
          c.type === 'reference' &&
          !c.virtual &&
          !c.computed &&
          c.target === table.name,
      )
      .map((column) => ({ table: other, column })),
  );
