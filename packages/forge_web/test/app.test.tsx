import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { IconDashboard } from '@tabler/icons-react';
import { describe, expect, it } from 'vitest';

import { FakeResponse } from '../src/testing';
import { fakeApi, opportunite, startApp } from './support';

const user = () => userEvent.setup();

describe('application', () => {
  it('connexion puis tableau de bord', async () => {
    const u = user();
    startApp({ signedIn: false, path: '/' });
    await u.type(await screen.findByLabelText(/E-mail/), 'admin@test');
    await u.type(screen.getByLabelText(/Mot de passe/), 'faux');
    await u.click(screen.getByRole('button', { name: 'Se connecter' }));
    expect(await screen.findByText('E-mail ou mot de passe incorrect')).toBeInTheDocument();
    await u.clear(screen.getByLabelText(/Mot de passe/));
    await u.type(screen.getByLabelText(/Mot de passe/), 'secret');
    await u.click(screen.getByRole('button', { name: 'Se connecter' }));
    expect(await screen.findByText(/Bonjour, Ada/)).toBeInTheDocument();
    // Indicateurs : nombre d'enregistrements par table.
    expect((await screen.findAllByText('2')).length).toBeGreaterThan(0);
  });

  it('liste : valeurs mises en forme, pastilles, tri', async () => {
    const u = user();
    const api = startApp({ path: '/data/entreprise' });
    expect(await screen.findByText('Acme')).toBeInTheDocument();
    expect(screen.getByText('Industrie')).toBeInTheDocument();
    expect(screen.getByText(/^1.500$/)).toBeInTheDocument();
    await u.click(screen.getByRole('button', { name: /Nom/ }));
    await waitFor(() =>
      expect(api.requestsTo('GET', '/api/entreprise').at(-1)!.url.searchParams.get('sort')).toBe('nom'),
    );
  });

  it('fiche : référence résolue, liste liée, colonne masquée absente', async () => {
    startApp({ path: '/data/opportunite/10' });
    expect(await screen.findByRole('heading', { name: 'Contrat cadre' })).toBeInTheDocument();
    expect(await screen.findByRole('link', { name: 'Acme' })).toBeInTheDocument();
    expect(screen.getByText(/^1.500,5$/)).toBeInTheDocument();
    expect(screen.queryByText('Notes')).not.toBeInTheDocument();
  });

  it('entreprise : opportunités liées filtrées sur la référence', async () => {
    const api = startApp({ path: '/data/entreprise/1' });
    expect(await screen.findByText('Contrat cadre')).toBeInTheDocument();
    expect(api.requestsTo('GET', '/api/opportunite').at(-1)!.url.searchParams.get('entreprise')).toBe('1');
  });

  it('création : validation, erreurs du serveur, enregistrement', async () => {
    const u = user();
    const api = fakeApi();
    let posted: unknown = null;
    let reject = true;
    api.on('POST', '/api/opportunite', (request) => {
      posted = request.body;
      return reject
        ? new FakeResponse(422, { error: { code: 'validation', message: 'invalide', fields: { titre: ['titre déjà utilisé'] } } })
        : opportunite(12, 'Nouveau contrat');
    });
    api.on('GET', '/api/opportunite/12', () => opportunite(12, 'Nouveau contrat'));
    startApp({ api, path: '/data/opportunite/new' });
    await u.click(await screen.findByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findAllByText('Valeur obligatoire')).toHaveLength(3);
    expect(posted).toBeNull();

    await u.type(screen.getByRole('textbox', { name: /Titre/ }), 'Nouveau contrat');
    await u.type(screen.getByRole('textbox', { name: /Montant/ }), '2 500,5');
    // Champ de saisie du sélecteur (le libellé désigne aussi sa liste).
    await u.click(screen.getAllByLabelText(/Entreprise/).find((el) => el.tagName === 'INPUT')!);
    await u.click(await screen.findByRole('option', { name: 'Globex', hidden: true }));
    await u.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() =>
      expect(posted).toEqual({ titre: 'Nouveau contrat', entreprise: 2, montant: '2500.5', etape: 'prospect' }),
    );
    expect(await screen.findByText('titre déjà utilisé')).toBeInTheDocument();

    reject = false;
    await u.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findByRole('heading', { name: 'Nouveau contrat' })).toBeInTheDocument();
  });

  it('modification : seules les colonnes changées sont envoyées', async () => {
    const u = user();
    const api = fakeApi();
    let patched: unknown = null;
    api.on('PATCH', '/api/opportunite/(\\d+)', (request) => {
      patched = request.body;
      return opportunite(10, 'Contrat révisé');
    });
    startApp({ api, path: '/data/opportunite/10/edit' });
    const title = await screen.findByRole('textbox', { name: /Titre/ });
    await u.clear(title);
    await u.type(title, 'Contrat révisé');
    await u.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() => expect(patched).toEqual({ titre: 'Contrat révisé' }));
  });

  it('filtre sur une énumération et recherche', async () => {
    const u = user();
    const api = startApp({ path: '/data/opportunite' });
    await screen.findByText('Contrat cadre');
    await u.click(screen.getByRole('button', { name: 'Filtrer' }));
    await u.click(await screen.findByRole('menuitem', { name: 'Etape', hidden: true }));
    const dialog = await screen.findByRole('dialog');
    await u.click(within(dialog).getByText('Gagne'));
    await u.click(within(dialog).getByRole('button', { name: 'Appliquer' }));
    expect(await screen.findByText(/Etape : Gagne/)).toBeInTheDocument();
    await waitFor(() =>
      expect(api.requestsTo('GET', '/api/opportunite').at(-1)!.url.searchParams.get('etape[in]')).toBe('gagne'),
    );
    await u.type(screen.getByRole('textbox', { name: 'Rechercher' }), 'cadre');
    await waitFor(() => {
      const params = api.requestsTo('GET', '/api/opportunite').at(-1)!.url.searchParams;
      expect(params.get('q')).toBe('cadre');
      expect(params.get('etape[in]')).toBe('gagne');
    });
  });

  it('statistiques et calendrier', async () => {
    const u = user();
    const api = startApp({ path: '/data/opportunite' });
    await screen.findByText('Contrat cadre');
    await u.click(screen.getByText('Statistiques'));
    expect(await screen.findAllByText(/^3.001$/)).not.toHaveLength(0);
    expect(screen.getAllByText('Prospect').length).toBeGreaterThan(0);
    await u.click(screen.getByText('Calendrier'));
    await waitFor(() => {
      const keys = [...api.requestsTo('GET', '/api/opportunite').at(-1)!.url.searchParams.keys()];
      expect(keys).toEqual(expect.arrayContaining(['date_cloture[gte]', 'date_cloture[lt]']));
    });
  });

  it('droits : un lecteur ne voit ni création ni administration', async () => {
    startApp({ api: fakeApi(['lecteur']), path: '/data/opportunite' });
    await screen.findByText('Contrat cadre');
    expect(screen.queryByRole('link', { name: 'Nouveau' })).not.toBeInTheDocument();
    expect(screen.queryByText('Utilisateurs')).not.toBeInTheDocument();
  });

  it('personnalisation : libellés, bloc de fiche, page', async () => {
    const u = user();
    startApp({
      path: '/data/entreprise/1',
      customization: {
        enumLabels: { 'entreprise.secteur': { industrie: 'Industrie lourde' } },
        detailSections: { entreprise: [({ record }) => <p>Bloc de {String(record.nom)}</p>] },
        pages: [{ path: 'tableau', label: 'Mon tableau', icon: IconDashboard, component: () => <p>Contenu perso</p> }],
      },
    });
    expect(await screen.findByText('Bloc de Acme')).toBeInTheDocument();
    expect(screen.getByText('Industrie lourde')).toBeInTheDocument();
    await u.click(screen.getByRole('link', { name: 'Mon tableau' }));
    expect(await screen.findByText('Contenu perso')).toBeInTheDocument();
  });
});
