/**
 * Aide aux tests des applications forge : une API en mémoire qui répond aux
 * routes déclarées par le test, et un client déjà connecté.
 */
import { ForgeClient, type Json } from './api/client';
import { MemoryStore } from './api/store';

/** Réponse explicite d'une route (statut autre que `200`). */
export class FakeResponse {
  constructor(
    readonly status: number,
    readonly body?: unknown,
  ) {}
}

export interface FakeRequest {
  method: string;
  url: URL;
  body: unknown;
  headers: Record<string, string>;
}

/** Traite une requête ; retourne le corps JSON, ou une `FakeResponse`. */
export type FakeHandler = (request: FakeRequest, match: RegExpExecArray) => unknown;

const error = (status: number, code: string, message: string) =>
  new FakeResponse(status, { error: { code, message } });

/**
 * API simulée : routes d'authentification intégrées (mot de passe `secret`),
 * autres routes déclarées par `on` ou `records`.
 */
export class FakeApi {
  /** Requêtes reçues, dans l'ordre. */
  readonly requests: FakeRequest[] = [];
  private readonly routes: [string, RegExp, FakeHandler][] = [];

  constructor(readonly user: Json = { id: 1, email: 'admin@test', display_name: null, active: true, roles: ['admin'] }) {
    this.on('POST', '/api/auth/login', (request) =>
      (request.body as Json).password === 'secret'
        ? this.session()
        : error(401, 'unauthorized', 'authentification requise'),
    );
    this.on('POST', '/api/auth/refresh', () => this.session());
    this.on('POST', '/api/auth/logout', () => new FakeResponse(204));
    this.on('GET', '/api/auth/me', () => this.user);
  }

  /**
   * Déclare une route ; `path` est une expression régulière sur le chemin
   * entier (`/api/contact/(\\d+)`). Les routes déclarées en dernier priment.
   */
  on(method: string, path: string, handler: FakeHandler) {
    this.routes.unshift([method, new RegExp(`^${path}$`), handler]);
  }

  /** Liste paginée (filtre `id[in]` compris) et lecture par identifiant. */
  records(table: string, records: Json[]) {
    this.on('GET', `/api/${table}`, ({ url }) => {
      const ids = url.searchParams.get('id[in]')?.split(',').map(Number);
      const matching = ids ? records.filter((r) => ids.includes(r.id as number)) : records;
      const page = Number(url.searchParams.get('page') ?? 1);
      const perPage = Number(url.searchParams.get('per_page') ?? 25);
      return {
        data: matching.slice((page - 1) * perPage, page * perPage),
        page,
        per_page: perPage,
        total: matching.length,
      };
    });
    this.on('GET', `/api/${table}/(\\d+)`, (_, match) => {
      const found = records.find((r) => r.id === Number(match[1]));
      return found ?? error(404, 'not_found', 'enregistrement introuvable');
    });
  }

  /** `fetch` de l'API simulée. */
  readonly fetch: typeof fetch = async (input, init) => {
    const url = new URL(String(input), 'http://api.test');
    const text = typeof init?.body === 'string' ? init.body : null;
    const request: FakeRequest = {
      method: init?.method ?? 'GET',
      url,
      body: text ? JSON.parse(text) : (init?.body ?? null),
      headers: (init?.headers as Record<string, string>) ?? {},
    };
    this.requests.push(request);
    for (const [method, path, handler] of this.routes) {
      const match = path.exec(url.pathname);
      if (method === request.method && match) {
        const result = handler(request, match);
        const [status, body] = result instanceof FakeResponse ? [result.status, result.body] : [200, result];
        return new Response(body === undefined ? null : JSON.stringify(body), {
          status,
          headers: { 'content-type': 'application/json' },
        });
      }
    }
    return new Response(JSON.stringify({ error: { code: 'not_found', message: url.pathname } }), { status: 404 });
  };

  /** Client de l'API simulée ; `signedIn` : une session est déjà enregistrée. */
  client(signedIn = true): ForgeClient {
    return new ForgeClient({
      baseUrl: 'http://api.test',
      fetch: this.fetch,
      store: new MemoryStore(signedIn ? { [ForgeClient.refreshTokenKey]: 'refresh' } : {}),
    });
  }

  /** Requêtes reçues pour `method` et `path`. */
  requestsTo(method: string, path: string): FakeRequest[] {
    return this.requests.filter((r) => r.method === method && r.url.pathname === path);
  }

  private session() {
    return { access_token: 'access', refresh_token: 'refresh', token_type: 'Bearer', expires_in: 900, user: this.user };
  }
}
