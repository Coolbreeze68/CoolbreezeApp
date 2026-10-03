/** Conversions entre le JSON de l'API et les types JS, et affichage des valeurs. */
import type { ForgeStrings } from './i18n';
import type { ColumnSchema, Label, TableSchema } from './schema';
import { titleColumns } from './schema';
import { isForgeFile } from './api/file';

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

/** Pourcentage saisi (`12,5` pour 12,5 %) → proportion au format de l'API (`"0.125"`). */
export function parsePercentInput(input: string): string | null {
  const percent = parseDecimalInput(input);
  return percent === null ? null : shiftDecimal(percent, -2);
}

/** Proportion de l'API (`"0.125"`) → pourcentage saisi (`"12.5"`). */
export const percentText = (ratio: string) => shiftDecimal(ratio, 2);

/** Décale la virgule d'un décimal en texte, sans arrondi binaire. */
function shiftDecimal(text: string, places: number): string {
  const negative = text.startsWith('-');
  const [whole, fraction = ''] = text.replace('-', '').split('.');
  let digits = whole + fraction;
  let point = whole.length + places;
  if (point < 0) {
    digits = '0'.repeat(-point) + digits;
    point = 0;
  }
  digits = digits.padEnd(point, '0');
  const integer = digits.slice(0, point).replace(/^0+(?=\d)/, '') || '0';
  const rest = digits.slice(point).replace(/0+$/, '');
  return `${negative ? '-' : ''}${integer}${rest ? `.${rest}` : ''}`;
}

/**
 * Saisie plausible pour une colonne `email`, `url` ou `phone` (le serveur fait
 * foi ; ce contrôle évite un aller-retour pour une faute évidente).
 */
export function isValidInput(type: string, text: string): boolean {
  switch (type) {
    case 'email':
      return /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(text);
    case 'url':
      return /^https?:\/\/[^\s/?#]+\S*$/.test(text);
    case 'phone':
      return /^\+?[\d\s().-]+$/.test(text) && (text.match(/\d/g) ?? []).length >= 6;
    default:
      return true;
  }
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
  private readonly percentFormat: Intl.NumberFormat;
  private readonly currencies = new Map<string, Intl.NumberFormat>();

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
    this.percentFormat = new Intl.NumberFormat(tag, { style: 'percent', maximumFractionDigits: 2 });
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

  /** Proportion en pourcentage : `0.255` → `25,5 %`. */
  percent = (ratio: number) => this.percentFormat.format(ratio);

  /** Montant dans la devise `currency` : `1 500,00 €`. */
  money(value: number, currency: string): string {
    let format = this.currencies.get(currency);
    if (!format) {
      format = new Intl.NumberFormat(this.locale.replace('_', '-'), { style: 'currency', currency });
      this.currencies.set(currency, format);
    }
    return format.format(value);
  }

  /** Symbole de la devise : `€`, `$`. */
  currencySymbol(currency: string): string {
    return (
      new Intl.NumberFormat(this.locale.replace('_', '-'), { style: 'currency', currency })
        .formatToParts(0)
        .find((p) => p.type === 'currency')?.value ?? currency
    );
  }

  /** Taille de fichier : `820 o`, `12 Ko`, `3,4 Mo`. */
  fileSize(bytes: number): string {
    const units = this.strings.sizeUnits;
    let size = bytes;
    let unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
      size /= 1024;
      unit++;
    }
    const shown = unit === 0 || size >= 10 ? Math.round(size) : Math.round(size * 10) / 10;
    return `${this.number(shown)} ${units[unit]}`;
  }

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
      case 'percent': {
        const n = Number(json);
        return Number.isNaN(n) ? String(json) : this.percent(n);
      }
      case 'money': {
        const n = Number(json);
        return Number.isNaN(n) ? String(json) : this.money(n, column.currency ?? 'EUR');
      }
      case 'rating':
        return `${String(json)}/${column.max ?? 5}`;
      case 'file':
      case 'image':
        return isForgeFile(json) ? json.name : '';
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
