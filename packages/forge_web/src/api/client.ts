import { BrowserStore, type KeyValueStore } from './store';
import { TableClient } from './table';

export type Json = Record<string, unknown>;

/** Ligne refusée d'un import CSV (la ligne d'en-tête est la ligne 1). */
export interface ImportLineError {
  line: number;
  message: string;
  fields: Record<string, string[]>;
}

/** Erreur renvoyée par l'API : `{ "error": { code, message, fields?, lines? } }`. */
export class ApiError extends Error {
  constructor(
    /** Statut HTTP (`0` si le serveur est injoignable). */
    readonly status: number,
    readonly code: string,
    message: string,
    /** Erreurs par champ d'une erreur de validation. */
    readonly fields: Record<string, string[]> = {},
    /** Erreurs par ligne d'un import refusé. */
    readonly lines: ImportLineError[] = [],
  ) {
    super(message);
    this.name = 'ApiError';
  }

  get isUnauthorized() {
    return this.status === 401;
  }

  static async fromResponse(response: Response): Promise<ApiError> {
    const body: unknown = await response.json().catch(() => null);
    const error =
      body && typeof body === 'object' && 'error' in body
        ? ((body as { error: Json }).error ?? {})
        : {};
    return new ApiError(
      response.status,
      (error.code as string) ?? `http_${response.status}`,
      (error.message as string) ?? response.statusText,
      (error.fields as Record<string, string[]>) ?? {},
      ((error.lines as Json[]) ?? []).map((line) => ({
        line: line.line as number,
        message: (line.message as string) ?? '',
        fields: (line.fields as Record<string, string[]>) ?? {},
      })),
    );
  }
}

/** Compte connecté. */
export interface ForgeUser {
  id: number;
  email: string;
  display_name: string | null;
  active: boolean;
  roles: string[];
}

export const userName = (user: ForgeUser) =>
  user.display_name?.trim() ? user.display_name : user.email;

export interface ForgeClientOptions {
  /** Adresse de l'API ; vide : l'origine de la page. */
  baseUrl?: string;
  store?: KeyValueStore;
  fetch?: typeof fetch;
}

/**
 * Client de l'API d'une application forge : session (jeton d'accès renouvelé
 * automatiquement) et requêtes JSON. Prévient ses abonnés à l'ouverture et à
 * la fermeture de la session.
 */
export class ForgeClient {
  /** Clé du jeton de rafraîchissement dans le stockage. */
  static readonly refreshTokenKey = 'forge.refresh_token';

  readonly baseUrl: string;
  readonly store: KeyValueStore;
  private readonly fetcher: typeof fetch;
  private accessToken: string | null = null;
  private refreshToken: string | null = null;
  private currentUser: ForgeUser | null = null;
  private refreshing: Promise<boolean> | null = null;
  private readonly listeners = new Set<() => void>();

  constructor(options: ForgeClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? '').replace(/\/$/, '');
    this.store = options.store ?? new BrowserStore();
    this.fetcher = options.fetch ?? ((...args) => fetch(...args));
  }

  get user(): ForgeUser | null {
    return this.currentUser;
  }

  get signedIn(): boolean {
    return this.currentUser !== null;
  }

  /** S'abonne aux changements de session ; retourne la désinscription. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Opérations sur une table, enregistrements en JSON. */
  table(name: string): TableClient<Json> {
    return new TableClient(this, name, (json) => json);
  }

  async signIn(email: string, password: string): Promise<ForgeUser> {
    const session = await this.send('POST', '/api/auth/login', {
      body: { email, password },
      authenticated: false,
    });
    return this.open(session as Json);
  }

  /** Reprend la session enregistrée, s'il y en a une encore valide. */
  async restore(): Promise<boolean> {
    this.refreshToken = this.store.read(ForgeClient.refreshTokenKey);
    return this.refreshToken !== null && this.refresh();
  }

  async signOut(): Promise<void> {
    const token = this.refreshToken;
    if (token) {
      try {
        await this.send('POST', '/api/auth/logout', {
          body: { refresh_token: token },
          authenticated: false,
        });
      } catch {
        // La session locale est fermée même si le serveur ne répond pas.
      }
    }
    this.close();
  }

  /** Recharge le compte connecté (rôles, nom). */
  async reloadUser(): Promise<ForgeUser> {
    this.currentUser = (await this.get('/api/auth/me')) as ForgeUser;
    this.notify();
    return this.currentUser;
  }

  get(path: string, query?: Record<string, string>): Promise<unknown> {
    return this.send('GET', path, { query });
  }

  post(path: string, body: unknown): Promise<unknown> {
    return this.send('POST', path, { body });
  }

  put(path: string, body: unknown): Promise<unknown> {
    return this.send('PUT', path, { body });
  }

  patch(path: string, body: unknown): Promise<unknown> {
    return this.send('PATCH', path, { body });
  }

  async delete(path: string): Promise<void> {
    await this.send('DELETE', path, {});
  }

  /** Corps brut d'une réponse (export CSV). */
  async getBlob(path: string, query?: Record<string, string>): Promise<Blob> {
    const response = await this.authenticated(() => ({
      url: this.url(path, query),
      init: { method: 'GET' },
    }));
    return response.blob();
  }

  /** Envoie un corps brut (import CSV) et décode la réponse JSON. */
  async postRaw(path: string, body: Blob, contentType = 'text/csv'): Promise<unknown> {
    const response = await this.authenticated(() => ({
      url: this.url(path),
      init: { method: 'POST', body, headers: { 'content-type': contentType } },
    }));
    return decode(response);
  }

  url(path: string, query?: Record<string, string>): string {
    const search = query && Object.keys(query).length ? `?${new URLSearchParams(query)}` : '';
    return `${this.baseUrl}${path}${search}`;
  }

  private async send(
    method: string,
    path: string,
    options: { query?: Record<string, string>; body?: unknown; authenticated?: boolean },
  ): Promise<unknown> {
    const build = () => {
      const headers: Record<string, string> = { accept: 'application/json' };
      const init: RequestInit = { method, headers };
      if (options.body !== undefined) {
        headers['content-type'] = 'application/json';
        init.body = JSON.stringify(options.body);
      }
      return { url: this.url(path, options.query), init };
    };
    const response =
      options.authenticated === false
        ? await this.execute(build())
        : await this.authenticated(build);
    return decode(response);
  }

  /** Requête avec le jeton d'accès ; sur `401`, renouvelle la session une fois et rejoue. */
  private async authenticated(
    build: () => { url: string; init: RequestInit },
  ): Promise<Response> {
    const attempt = () => {
      const request = build();
      if (this.accessToken) {
        request.init.headers = {
          ...(request.init.headers as Record<string, string>),
          authorization: `Bearer ${this.accessToken}`,
        };
      }
      return this.execute(request);
    };
    try {
      return await attempt();
    } catch (error) {
      if (!(error instanceof ApiError) || !error.isUnauthorized || !this.refreshToken) throw error;
      if (!(await this.refresh())) throw error;
      return attempt();
    }
  }

  private async execute({ url, init }: { url: string; init: RequestInit }): Promise<Response> {
    let response: Response;
    try {
      response = await this.fetcher(url, init);
    } catch (error) {
      throw new ApiError(0, 'network', String(error));
    }
    if (response.status >= 400) throw await ApiError.fromResponse(response);
    return response;
  }

  /** Renouvelle la session ; les appels simultanés partagent le même renouvellement. */
  private refresh(): Promise<boolean> {
    this.refreshing ??= (async () => {
      try {
        const session = await this.send('POST', '/api/auth/refresh', {
          body: { refresh_token: this.refreshToken },
          authenticated: false,
        });
        this.open(session as Json);
        return true;
      } catch (error) {
        // Serveur injoignable : la session reste valable pour un nouvel essai.
        if (!(error instanceof ApiError) || error.status !== 0) this.close();
        return false;
      } finally {
        this.refreshing = null;
      }
    })();
    return this.refreshing;
  }

  private open(session: Json): ForgeUser {
    this.accessToken = session.access_token as string;
    this.refreshToken = session.refresh_token as string;
    this.currentUser = session.user as ForgeUser;
    this.store.write(ForgeClient.refreshTokenKey, this.refreshToken);
    this.notify();
    return this.currentUser;
  }

  private close() {
    const wasSignedIn = this.signedIn;
    this.accessToken = this.refreshToken = null;
    this.currentUser = null;
    this.store.write(ForgeClient.refreshTokenKey, null);
    if (wasSignedIn) this.notify();
  }

  private notify() {
    for (const listener of this.listeners) listener();
  }
}

async function decode(response: Response): Promise<unknown> {
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}
