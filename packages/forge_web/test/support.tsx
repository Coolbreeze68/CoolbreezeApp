import { render } from '@testing-library/react';

import { type AppSchema, type ForgeCustomization, ForgeApp, type Json } from '../src';
import { FakeApi } from '../src/testing';

/** Schéma de test, sur le modèle du CRM d'exemple. */
export const schema: AppSchema = {
  name: 'crm_test',
  default_locale: 'fr',
  locales: ['fr', 'en'],
  roles: ['admin', 'commercial', 'lecteur'],
  parameters: [{ name: 'tva', type: 'decimal', label: { fr: 'TVA', en: 'VAT' } }],
  tables: [
    {
      name: 'entreprise',
      label: { fr: 'Entreprise', en: 'Company' },
      columns: [
        { name: 'nom', type: 'string', label: { fr: 'Nom', en: 'Name' }, required: true, title_field: true },
        { name: 'secteur', type: 'enum', values: ['industrie', 'services'] },
        { name: 'pipeline', type: 'decimal', computed: true },
      ],
      rules: [{ roles: ['commercial', 'lecteur'], operations: ['read'] }],
    },
    {
      name: 'opportunite',
      label: { fr: 'Opportunité', en: 'Opportunity' },
      columns: [
        { name: 'titre', type: 'string', required: true, title_field: true },
        { name: 'entreprise', type: 'reference', required: true, target: 'entreprise' },
        { name: 'montant', type: 'decimal', required: true },
        { name: 'etape', type: 'enum', values: ['prospect', 'gagne'], default: 'prospect' },
        { name: 'date_cloture', type: 'date' },
        { name: 'notes', type: 'text', hidden: true },
      ],
      calendar: { start: 'date_cloture' },
      stats: { fields: ['montant'], group_by: 'etape' },
      rules: [
        { roles: ['commercial'], operations: ['read', 'create'] },
        { roles: ['commercial'], operations: ['update', 'delete'], conditional: true },
        { roles: ['lecteur'], operations: ['read'] },
      ],
    },
  ],
};

export const entreprises: Json[] = [
  { id: 1, nom: 'Acme', secteur: 'industrie', pipeline: '1500' },
  { id: 2, nom: 'Globex', secteur: 'services', pipeline: null },
];

export const opportunite = (id: number, titre: string, date: string | null = null): Json => ({
  id,
  titre,
  entreprise: 1,
  montant: '1500.5',
  etape: 'prospect',
  date_cloture: date,
  notes: null,
  created_at: '2026-10-01T08:00:00Z',
  updated_at: '2026-10-01T08:00:00Z',
});

const aggregate = {
  fields: ['montant'],
  group_by: 'etape',
  groups: [{ key: 'prospect', count: 2, montant: { sum: '3001', avg: '1500.5', min: '1500.5', max: '1500.5' } }],
  total: { count: 2, montant: { sum: '3001', avg: '1500.5', min: '1500.5', max: '1500.5' } },
};

/** API simulée avec les données de test. */
export function fakeApi(roles: string[] = ['admin']) {
  const api = new FakeApi({ id: 1, email: 'admin@test', display_name: 'Ada', active: true, roles });
  api.records('entreprise', entreprises);
  api.records('opportunite', [opportunite(10, 'Contrat cadre', '2026-10-15'), opportunite(11, 'Extension')]);
  api.on('GET', '/api/opportunite/aggregate', () => aggregate);
  return api;
}

/** Lance l'application à `path`. */
export function startApp({
  api = fakeApi(),
  path = '/',
  signedIn = true,
  customization,
  appSchema = schema,
}: { api?: FakeApi; path?: string; signedIn?: boolean; customization?: ForgeCustomization; appSchema?: AppSchema } = {}) {
  // Sans animations : menus et fenêtres s'ouvrent immédiatement.
  const instant = { defaultProps: { transitionProps: { duration: 0 } } };
  const theme = { components: { Menu: instant, Popover: instant, Modal: instant, Combobox: instant, Select: { defaultProps: { comboboxProps: { transitionProps: { duration: 0 } } } } } };
  render(
    <ForgeApp
      schema={appSchema}
      client={api.client(signedIn)}
      customization={{ ...customization, theme }}
      initialPath={path}
    />,
  );
  return api;
}
