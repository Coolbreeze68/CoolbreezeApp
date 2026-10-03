/** Stockage persistant de petites valeurs : jeton de rafraîchissement, langue. */
export interface KeyValueStore {
  read(key: string): string | null;
  /** Écrit `value`, ou supprime la clé si `value` est `null`. */
  write(key: string, value: string | null): void;
}

/**
 * Stockage du navigateur (`localStorage`). Indisponible (navigation privée
 * stricte, stockage bloqué) : les valeurs ne sont simplement pas conservées.
 */
export class BrowserStore implements KeyValueStore {
  read(key: string): string | null {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  }

  write(key: string, value: string | null): void {
    try {
      if (value === null) window.localStorage.removeItem(key);
      else window.localStorage.setItem(key, value);
    } catch {
      // Stockage refusé : la session ne sera pas reprise au rechargement.
    }
  }
}

/** Stockage en mémoire (tests, sessions éphémères). */
export class MemoryStore implements KeyValueStore {
  readonly values: Map<string, string>;

  constructor(values: Record<string, string> = {}) {
    this.values = new Map(Object.entries(values));
  }

  read(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  write(key: string, value: string | null): void {
    if (value === null) this.values.delete(key);
    else this.values.set(key, value);
  }
}
