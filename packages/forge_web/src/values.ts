/** Conversions entre le JSON de l'API et les types JS, et affichage des valeurs. */
import type { ForgeStrings } from './i18n';
import type { ColumnSchema, Label, TableSchema } from './schema';
import { titleColumns } from './schema';

// ------------------------------------------------------------- JSON → JS

/** `"2026-10-02"` → date locale à minuit. */
export function jsonToDate(json: unknown): Date | null {
  if (typeof json !== 'string') return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(json);
  return match ? new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])) : null;
}

/** Date-heure RFC 3339 → `Date`. */
export function jsonToDateTime(json: unknown): Date | null {
  if (typeof json !== 'string') return null;
  const date = new Date(json);
  return Number.isNaN(date.getTime()) ? null : date;
}

export const jsonToIds = (json: unknown): number[] =>
  Array.isArray(json) ? json.filter((id): id is number => typeof id === 'number') : [];

// ------------------------------------------------------------- JS → JSON

const pad = (n: number, width = 2) => String(n).padStart(width, '0');

/** Date locale → `"2026-10-02"`. */
export const dateToJson = (date: Date) =>
  `${pad(date.getFullYear(), 4)}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;

export const dateTimeToJson = (date: Date) => date.toISOString();

/** Décimal saisi par l'utilisateur (`1 234,5` ou `1234.5`) → texte de l'API, ou `null`. */
export function parseDecimalInput(input: string): string | null {
  const normalized = input.replace(/[\s\u00a0\u202f]/g, '').replace(',', '.');
  if (!/^-?\d+(\.\d+)?$/.test(normalized)) return null;
  return normalized.replace(/(\.\d*?)0+$/, '$1').replace(/\.$/, '');
}

/** Entier saisi par l'utilisateur, espaces de groupement tolérés. */
export function parseIntegerInput(input: string): number | null {
  const normalized = input.replace(/[\s\u00a0\u202f]/g, '');
  return /^-?\d+$/.test(normalized) ? Number(normalized) : null;
}

// -------------------------------------------------------------- Affichage

/** Libellés personnalisés des valeurs d'énumération : `table.colonne` → valeur → libellé. */
export type EnumLabels = Record<string, Record<string, Label>>;

/** `date_cloture` → `Date cloture`. */
export function humanize(name: string): string {
  const words = name.replace(/_/g, ' ').trim();
  return words ? words[0].toUpperCase() + words.slice(1) : name;
}

/** Mise en forme des valeurs dans la langue de l'interface. */
export class ValueFormat {
  private readonly integer: Intl.NumberFormat;
  private readonly decimal: Intl.NumberFormat;
  private readonly dateFormat: Intl.DateTimeFormat;
  private readonly dateTimeFormat: Intl.DateTimeFormat;
  private readonly timeFormat: Intl.DateTimeFormat;

  constructor(
    readonly locale: string,
    readonly fallbackLocale: string,
    readonly strings: ForgeStrings,
    readonly enumLabels: EnumLabels = {},
  ) {
    const tag = locale.replace('_', '-');
    this.integer = new Intl.NumberFormat(tag, { maximumFractionDigits: 0 });
    this.decimal = new Intl.NumberFormat(tag, { maximumFractionDigits: 4 });
    this.dateFormat = new Intl.DateTimeFormat(tag, { dateStyle: 'short' });
    this.dateTimeFormat = new Intl.DateTimeFormat(tag, { dateStyle: 'short', timeStyle: 'short' });
    this.timeFormat = new Intl.DateTimeFormat(tag, { timeStyle: 'short' });
  }

  label(label: Label | undefined, name: string): string {
    if (typeof label === 'string') return label;
    if (label) {
      return label[this.locale] ?? label[this.fallbackLocale] ?? Object.values(label)[0] ?? humanize(name);
    }
    return humanize(name);
  }

  tableLabel = (table: TableSchema) => this.label(table.label, table.name);

  columnLabel = (column: ColumnSchema) => this.label(column.label, column.name);

  enumLabel(table: TableSchema, column: ColumnSchema, value: string): string {
    const custom = this.enumLabels[`${table.name}.${column.name}`]?.[value];
    return custom ? this.label(custom, value) : humanize(value);
  }

  number = (value: number) =>
    Number.isInteger(value) ? this.integer.format(value) : this.decimal.format(value);

  date = (value: Date) => this.dateFormat.format(value);

  dateTime = (value: Date) => this.dateTimeFormat.format(value);

  time = (value: Date) => this.timeFormat.format(value);

  /** `1 h 30`, `45 min`, `2 h`. */
  duration(seconds: number): string {
    const negative = seconds < 0;
    const minutes = Math.floor(Math.abs(seconds) / 60);
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    const text = h === 0 ? `${m} min` : m === 0 ? `${h} h` : `${h} h ${pad(m)}`;
    return negative ? `-${text}` : text;
  }

  /**
   * Texte d'une valeur JSON de la colonne ; une référence s'affiche par son
   * identifiant (l'interface la remplace par l'intitulé de l'enregistrement).
   */
  format(table: TableSchema, column: ColumnSchema, json: unknown): string {
    if (json === null || json === undefined) return '';
    switch (column.type) {
      case 'integer':
      case 'decimal': {
        const n = Number(json);
        return Number.isNaN(n) ? String(json) : this.number(n);
      }
      case 'boolean':
        return json === true ? this.strings.yes : this.strings.no;
      case 'date': {
        const d = jsonToDate(json);
        return d ? this.date(d) : String(json);
      }
      case 'datetime': {
        const d = jsonToDateTime(json);
        return d ? this.dateTime(d) : String(json);
      }
      case 'duration':
        return typeof json === 'number' ? this.duration(json) : String(json);
      case 'enum':
        return this.enumLabel(table, column, String(json));
      case 'reference':
        return `#${json}`;
      case 'reference_list':
        return jsonToIds(json).map((id) => `#${id}`).join(', ');
      default:
        return String(json);
    }
  }

  /** Intitulé d'un enregistrement : ses colonnes `title_field`, sinon `#id`. */
  title(table: TableSchema, record: Record<string, unknown>): string {
    const parts = titleColumns(table)
      .map((c) => this.format(table, c, record[c.name]))
      .filter(Boolean);
    return parts.length ? parts.join(' ') : `#${record.id}`;
  }
}
