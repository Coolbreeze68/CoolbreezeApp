import { describe, expect, it } from 'vitest';

import { ApiError, ForgeClient, MemoryStore } from '../src';
import { FakeApi, FakeResponse } from '../src/testing';

describe('client', () => {
  it('connexion, session enregistrée puis reprise', async () => {
    const api = new FakeApi();
    const client = api.client(false);
    expect(await client.restore()).toBe(false);
    await expect(client.signIn('admin@test', 'faux')).rejects.toMatchObject({ status: 401 });
    const user = await client.signIn('admin@test', 'secret');
    expect(user.roles).toEqual(['admin']);
    expect(client.store.read(ForgeClient.refreshTokenKey)).toBe('refresh');
    const restored = new ForgeClient({ baseUrl: 'http://api.test', fetch: api.fetch, store: client.store });
    expect(await restored.restore()).toBe(true);
    await restored.signOut();
    expect(restored.signedIn).toBe(false);
    expect(client.store.read(ForgeClient.refreshTokenKey)).toBeNull();
  });

  it('jeton expiré : renouvelé une fois, requête rejouée', async () => {
    const api = new FakeApi();
    let calls = 0;
    api.on('GET', '/api/tag', () =>
      ++calls === 1 ? new FakeResponse(401, { error: { code: 'unauthorized', message: 'expiré' } }) : { data: [], page: 1, per_page: 25, total: 0 },
    );
    const client = api.client();
    await client.restore();
    expect((await client.table('tag').list()).total).toBe(0);
    expect(calls).toBe(2);
    expect(api.requestsTo('POST', '/api/auth/refresh')).toHaveLength(2);
  });

  it('erreurs de validation et d\'import décodées', async () => {
    const api = new FakeApi();
    api.on('POST', '/api/contact', () => new FakeResponse(422, { error: { code: 'validation', message: 'données invalides', fields: { nom: ['valeur obligatoire'] } } }));
    api.on('POST', '/api/contact/import', () => new FakeResponse(422, { error: { code: 'import', message: 'refusé', lines: [{ line: 3, code: 'validation', message: 'invalide', fields: { email: ['déjà utilisé'] } }] } }));
    const client = api.client();
    await client.restore();
    await expect(client.table('contact').create({})).rejects.toMatchObject({ code: 'validation', fields: { nom: ['valeur obligatoire'] } });
    const error = await client.table('contact').import(new Blob(['nom\n'])).catch((e: ApiError) => e);
    expect((error as ApiError).lines[0].line).toBe(3);
  });

  it('serveur injoignable : erreur réseau', async () => {
    const client = new ForgeClient({ fetch: () => Promise.reject(new TypeError('Failed to fetch')), store: new MemoryStore() });
    await expect(client.signIn('a@b.c', 'x')).rejects.toMatchObject({ status: 0 });
  });
});
