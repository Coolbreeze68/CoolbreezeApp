import type { ForgeClient } from './api/client';
import { MAX_PER_PAGE, oneOf } from './api/query';
import type { AppSchema } from './schema';
import { findTable } from './schema';
import type { ValueFormat } from './values';

/**
 * Intitulés des enregistrements visés par des références, chargés par lots :
 * les demandes faites pendant un même rendu partent en une requête par table.
 */
export class TitleCache {
  private readonly titles = new Map<string, Map<number, Promise<string>>>();
  private readonly pending = new Map<string, Map<number, (title: string) => void>>();

  constructor(
    private readonly client: ForgeClient,
    private readonly schema: AppSchema,
    private readonly format: ValueFormat,
  ) {}

  /** Intitulé d'un enregistrement, ou `#id` s'il est illisible. */
  title(table: string, id: number): Promise<string> {
    let known = this.titles.get(table);
    if (!known) this.titles.set(table, (known = new Map()));
    let title = known.get(id);
    if (!title) {
      let pending = this.pending.get(table);
      if (!pending) {
        this.pending.set(table, (pending = new Map()));
        queueMicrotask(() => void this.flush(table));
      }
      const waiting = pending;
      title = new Promise((resolve) => waiting.set(id, resolve));
      known.set(id, title);
    }
    return title;
  }

  /** Oublie les intitulés (après une écriture). */
  clear() {
    this.titles.clear();
  }

  private async flush(table: string) {
    const pending = this.pending.get(table) ?? new Map<number, (title: string) => void>();
    this.pending.delete(table);
    const schema = findTable(this.schema, table);
    const found = new Map<number, string>();
    const ids = [...pending.keys()];
    if (schema) {
      try {
        for (let i = 0; i < ids.length; i += MAX_PER_PAGE) {
          const chunk = ids.slice(i, i + MAX_PER_PAGE);
          const listing = await this.client.table(table).list({
            perPage: MAX_PER_PAGE,
            filters: [oneOf('id', chunk.map(String))],
          });
          for (const record of listing.items) {
            found.set(record.id as number, this.format.title(schema, record));
          }
        }
      } catch {
        // Table illisible pour cet utilisateur : intitulés par défaut.
      }
    }
    for (const [id, resolve] of pending) resolve(found.get(id) ?? `#${id}`);
  }
}
