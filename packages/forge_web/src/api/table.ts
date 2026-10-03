import type { ForgeClient, Json } from './client';
import type { ForgeFile } from './file';
import { type ListQuery, type Listing, MAX_PER_PAGE, pageCount, queryParameters } from './query';

/** Bilan d'un import CSV réussi. */
export interface ImportReport {
  created: number;
  updated: number;
}

/** Somme, moyenne, minimum et maximum d'une colonne. */
export interface Measure {
  sum: number | null;
  avg: number | null;
  min: number | null;
  max: number | null;
}

export interface AggregateGroup {
  /** Valeur de la colonne de regroupement (`null` : sans valeur). */
  key: unknown;
  count: number;
  measures: Record<string, Measure>;
}

export interface Aggregation {
  fields: string[];
  groupBy: string | null;
  groups: AggregateGroup[];
  /** Ensemble des enregistrements. */
  total: AggregateGroup;
}

/** Les décimaux arrivent en texte, les entiers en nombre. */
const toNumber = (value: unknown): number | null => {
  if (typeof value === 'number') return value;
  if (typeof value === 'string' && value.trim() !== '' && !Number.isNaN(Number(value))) {
    return Number(value);
  }
  return null;
};

function group(json: Json, fields: string[]): AggregateGroup {
  const measures: Record<string, Measure> = {};
  for (const field of fields) {
    const m = json[field] as Json | undefined;
    if (m) {
      measures[field] = {
        sum: toNumber(m.sum),
        avg: toNumber(m.avg),
        min: toNumber(m.min),
        max: toNumber(m.max),
      };
    }
  }
  return { key: json.key ?? null, count: json.count as number, measures };
}

/**
 * Opérations REST sur une table ; `decode` convertit un enregistrement JSON en
 * `T` (le JSON lui-même pour l'interface générique, un modèle typé de
 * `src/generated/models.ts` pour le code personnalisé).
 */
export class TableClient<T> {
  constructor(
    readonly client: ForgeClient,
    readonly table: string,
    readonly decode: (json: Json) => T,
  ) {}

  private get path() {
    return `/api/${this.table}`;
  }

  async list(query: ListQuery = {}): Promise<Listing<T>> {
    const json = (await this.client.get(this.path, queryParameters(query))) as Json;
    return {
      items: (json.data as Json[]).map(this.decode),
      page: json.page as number,
      perPage: json.per_page as number,
      total: json.total as number,
    };
  }

  /** Tous les enregistrements correspondant à `query`, page par page, dans la limite de `max`. */
  async listAll(query: ListQuery = {}, max = 1000): Promise<T[]> {
    const items: T[] = [];
    for (let page = 1; ; page++) {
      const listing = await this.list({ ...query, page, perPage: MAX_PER_PAGE });
      items.push(...listing.items);
      if (page >= pageCount(listing) || items.length >= max) return items;
    }
  }

  async read(id: number): Promise<T> {
    return this.decode((await this.client.get(`${this.path}/${id}`)) as Json);
  }

  async create(values: Json): Promise<T> {
    return this.decode((await this.client.post(this.path, values)) as Json);
  }

  /** Modification partielle : seules les colonnes de `values` changent. */
  async update(id: number, values: Json): Promise<T> {
    return this.decode((await this.client.patch(`${this.path}/${id}`, values)) as Json);
  }

  delete(id: number): Promise<void> {
    return this.client.delete(`${this.path}/${id}`);
  }

  async aggregate(fields: string[], groupBy?: string, query: ListQuery = {}): Promise<Aggregation> {
    const json = (await this.client.get(`${this.path}/aggregate`, {
      ...queryParameters(query, false),
      fields: fields.join(','),
      ...(groupBy ? { group_by: groupBy } : {}),
    })) as Json;
    const names = json.fields as string[];
    return {
      fields: names,
      groupBy: (json.group_by as string | null) ?? null,
      groups: ((json.groups as Json[]) ?? []).map((g) => group(g, names)),
      total: group(json.total as Json, names),
    };
  }

  /** Export CSV des enregistrements correspondant à `query` (sans pagination). */
  export(query: ListQuery = {}, delimiter = ','): Promise<Blob> {
    return this.client.getBlob(`${this.path}/export`, {
      ...queryParameters(query, false),
      delimiter,
    });
  }

  /** Import CSV, tout ou rien ; une erreur détaille les lignes refusées. */
  async import(csv: Blob): Promise<ImportReport> {
    return (await this.client.postRaw(`${this.path}/import`, csv)) as ImportReport;
  }

  /** Téléverse un fichier pour la colonne `file` ou `image` ; son `id` s'écrit ensuite dans la colonne. */
  async upload(column: string, file: File): Promise<ForgeFile> {
    return (await this.client.postRaw('/api/files', file, file.type || 'application/octet-stream', {
      table: this.table,
      column,
      name: file.name,
    })) as ForgeFile;
  }
}
