/** Paramètres de liste de l'API : pagination, tri, recherche, filtres. */

/** Opérateur de filtre (`colonne[op]=valeur`) ; `in` : parmi une liste, `null` : absent. */
export type FilterOp = 'eq' | 'ne' | 'lt' | 'lte' | 'gt' | 'gte' | 'like' | 'in' | 'null';

export interface Filter {
  column: string;
  op: FilterOp;
  /** Valeur au format de l'API (date ISO, nombre avec un point…). */
  value: string;
}

export const equals = (column: string, value: string): Filter => ({ column, op: 'eq', value });

export const oneOf = (column: string, values: readonly string[]): Filter => ({
  column,
  op: 'in',
  value: values.join(','),
});

export const filterKey = (f: Filter) => (f.op === 'eq' ? f.column : `${f.column}[${f.op}]`);

export interface Sort {
  column: string;
  descending?: boolean;
}

export interface ListQuery {
  page?: number;
  perPage?: number;
  sort?: Sort[];
  search?: string;
  filters?: Filter[];
}

/** Taille de page maximale acceptée par l'API. */
export const MAX_PER_PAGE = 100;
export const DEFAULT_PER_PAGE = 25;

/** Paramètres d'URL ; sans pagination pour les agrégats et l'export. */
export function queryParameters(query: ListQuery, paging = true): Record<string, string> {
  const params: Record<string, string> = {};
  if (paging) {
    params.page = String(query.page ?? 1);
    params.per_page = String(query.perPage ?? DEFAULT_PER_PAGE);
  }
  if (query.sort?.length) {
    params.sort = query.sort.map((s) => (s.descending ? `-${s.column}` : s.column)).join(',');
  }
  const search = query.search?.trim();
  if (search) params.q = search;
  for (const filter of query.filters ?? []) params[filterKey(filter)] = filter.value;
  return params;
}

/** Ajoute des filtres, remplaçant ceux de même clé, et revient à la première page. */
export function withFilters(query: ListQuery, added: Filter[]): ListQuery {
  const keys = new Set(added.map(filterKey));
  return {
    ...query,
    page: 1,
    filters: [...(query.filters ?? []).filter((f) => !keys.has(filterKey(f))), ...added],
  };
}

/** Clé stable d'une requête (dépendances des effets). */
export const queryKey = (query: ListQuery) =>
  JSON.stringify(Object.entries(queryParameters(query)).sort());

/** Page de résultats. */
export interface Listing<T> {
  items: T[];
  page: number;
  perPage: number;
  total: number;
}

export const pageCount = (listing: Listing<unknown>) =>
  listing.total === 0 ? 1 : Math.ceil(listing.total / listing.perPage);
