import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AppSchema, Json } from '../src';
import { FakeApi } from '../src/testing';
import { startApp } from './support';

const user = () => userEvent.setup();

/** Une table par modèle de champ, et une table cible de référence. */
const models: AppSchema = {
  name: 'modeles',
  default_locale: 'fr',
  locales: ['fr'],
  roles: ['admin'],
  tables: [
    {
      name: 'fiche',
      columns: [
        { name: 'nom', type: 'string', required: true, title_field: true },
        { name: 'couleur', type: 'color' },
        { name: 'courriel', type: 'email' },
        { name: 'note', type: 'rating', max: 5 },
        { name: 'remise', type: 'percent' },
        { name: 'budget', type: 'money', currency: 'EUR' },
        { name: 'description', type: 'markdown' },
        { name: 'contrat', type: 'file', max_size: 1 },
        { name: 'societe', type: 'reference', target: 'societe' },
      ],
    },
    { name: 'societe', columns: [{ name: 'nom', type: 'string', required: true, title_field: true }] },
  ],
};

const contrat = {
  id: '0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a',
  name: 'contrat.pdf',
  size: 2048,
  content_type: 'application/pdf',
  url: '/api/files/0b9f3c1e?expires=1&signature=x',
};

const fiche: Json = {
  id: 1,
  nom: 'Acme',
  couleur: '#3366ff',
  courriel: 'contact@acme.fr',
  note: 4,
  remise: '0.125',
  budget: '1500',
  description: 'Client **important**',
  contrat,
  societe: null,
  created_at: '2026-10-01T08:00:00Z',
  updated_at: '2026-10-01T08:00:00Z',
};

function modelsApi() {
  const api = new FakeApi({ id: 1, email: 'admin@test', display_name: 'Ada', active: true, roles: ['admin'] });
  api.records('fiche', [fiche]);
  api.records('societe', [{ id: 7, nom: 'Initech SA' }]);
  return api;
}

describe('modèles de champ', () => {
  it('fiche : couleur, lien, étoiles, %, montant, Markdown, fichier', async () => {
    startApp({ api: modelsApi(), appSchema: models, path: '/data/fiche/1' });
    expect(await screen.findByText('#3366ff')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /contact@acme\.fr/ })).toHaveAttribute('href', 'mailto:contact@acme.fr');
    expect(screen.getByLabelText('4/5')).toBeInTheDocument();
    expect(screen.getByText(/^12,5\s%$/)).toBeInTheDocument();
    expect(screen.getByText(/^1\s500,00\s€$/)).toBeInTheDocument();
    expect(screen.getByText('important').tagName).toBe('STRONG');
    expect(screen.getByRole('link', { name: /contrat\.pdf/ })).toHaveAttribute(
      'href',
      'http://api.test/api/files/0b9f3c1e?expires=1&signature=x',
    );
    expect(screen.getByText(/^2\sKo$/)).toBeInTheDocument();
  });

  it('saisie : pourcentage, e-mail, fichier, référence créée', async () => {
    const u = user();
    const api = modelsApi();
    let posted: unknown = null;
    api.on('POST', '/api/fiche', (request) => {
      posted = request.body;
      return fiche;
    });
    api.on('POST', '/api/societe', () => ({ id: 7, nom: 'Initech SA' }));
    api.on('POST', '/api/files', () => contrat);
    startApp({ api, appSchema: models, path: '/data/fiche/new' });

    await u.type(await screen.findByRole('textbox', { name: /^Nom/ }), 'Initech');
    await u.type(screen.getByRole('textbox', { name: /Remise/ }), '12,5');
    await u.type(screen.getByRole('textbox', { name: /Courriel/ }), 'non');
    await u.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findByText('Valeur invalide')).toBeInTheDocument();
    expect(posted).toBeNull();
    await u.clear(screen.getByRole('textbox', { name: /Courriel/ }));
    await u.type(screen.getByRole('textbox', { name: /Courriel/ }), 'it@initech.com');

    // Fichier : envoyé dès le choix.
    await u.upload(screen.getByLabelText('Contrat'), new File(['%PDF'], 'contrat.pdf', { type: 'application/pdf' }));
    expect(await screen.findByText('contrat.pdf')).toBeInTheDocument();
    const upload = api.requestsTo('POST', '/api/files')[0];
    expect(upload.url.searchParams.get('column')).toBe('contrat');

    // Référence : création dans une fenêtre, sans quitter le formulaire.
    await u.click(screen.getByRole('button', { name: 'Nouveau : Societe' }));
    const dialog = await screen.findByRole('dialog');
    await u.type(within(dialog).getByRole('textbox', { name: /Nom/ }), 'Initech SA');
    await u.click(within(dialog).getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    await u.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() =>
      expect(posted).toEqual({
        nom: 'Initech',
        courriel: 'it@initech.com',
        remise: '0.125',
        contrat,
        societe: 7,
      }),
    );
  });
});
