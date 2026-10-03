import { describe, expect, it } from 'vitest';

import { allows, fr, en, findTable, queryParameters, relatedLists, ValueFormat, withFilters } from '../src';
import { datePresets, dateToJson, jsonToDate, localDateTimeText, parseDecimalInput, parseIntegerInput } from '../src';
import { schema } from './support';

const opportunites = findTable(schema, 'opportunite')!;
const column = (name: string) => opportunites.columns.find((c) => c.name === name)!;

describe('valeurs', () => {
  const frFormat = new ValueFormat('fr', 'fr', fr, { 'opportunite.etape': { gagne: { fr: 'Gagnée', en: 'Won' } } });
  const enFormat = new ValueFormat('en', 'fr', en, { 'opportunite.etape': { gagne: { fr: 'Gagnée', en: 'Won' } } });

  it('met en forme selon la langue', () => {
    expect(frFormat.format(opportunites, column('montant'), '1234.5')).toBe('1 234,5');
    expect(enFormat.format(opportunites, column('montant'), '1234.5')).toBe('1,234.5');
    expect(frFormat.format(opportunites, column('date_cloture'), '2026-10-02')).toBe('02/10/2026');
    expect(frFormat.format(opportunites, column('etape'), 'gagne')).toBe('Gagnée');
    expect(enFormat.format(opportunites, column('etape'), 'gagne')).toBe('Won');
    expect(frFormat.format(opportunites, column('etape'), 'prospect')).toBe('Prospect');
    expect(frFormat.duration(5400)).toBe('1 h 30');
    expect(frFormat.duration(2700)).toBe('45 min');
    expect(frFormat.columnLabel(column('date_cloture'))).toBe('Date cloture');
    expect(frFormat.title(opportunites, { id: 3, titre: null })).toBe('#3');
  });

  it('convertit et lit les saisies', () => {
    expect(dateToJson(jsonToDate('2026-01-05T10:00:00Z')!)).toBe('2026-01-05');
    expect(parseDecimalInput('1 234,50')).toBe('1234.5');
    expect(parseDecimalInput('douze')).toBeNull();
    expect(parseIntegerInput('1 000')).toBe(1000);
    expect(parseIntegerInput('1,5')).toBeNull();
  });
});

describe('schéma et requêtes', () => {
  it('droits par rôle, admin a tout', () => {
    expect(allows(opportunites, ['admin'], 'delete')).toBe(true);
    expect(allows(opportunites, ['commercial'], 'update')).toBe(true);
    expect(allows(opportunites, ['lecteur'], 'create')).toBe(false);
  });

  it('listes liées', () => {
    const related = relatedLists(schema, findTable(schema, 'entreprise')!);
    expect(related.map((r) => `${r.table.name}.${r.column.name}`)).toEqual(['opportunite.entreprise']);
  });

  it('paramètres de liste', () => {
    const query = withFilters(
      { page: 2, sort: [{ column: 'montant', descending: true }, { column: 'titre' }], search: ' acme ' },
      [{ column: 'montant', op: 'gte', value: '1000' }],
    );
    expect(queryParameters(query)).toEqual({
      page: '1',
      per_page: '25',
      sort: '-montant,titre',
      q: 'acme',
      'montant[gte]': '1000',
    });
    expect(queryParameters(query, false)).not.toHaveProperty('page');
  });
});

describe('datePresets', () => {
  const now = new Date(2026, 9, 31, 14, 27, 45);

  it('propose aujourd’hui, demain et dans une semaine pour une date', () => {
    expect(datePresets(false, now).map((p) => [p.label, dateToJson(p.date)])).toEqual([
      ['today', '2026-10-31'],
      ['tomorrow', '2026-11-01'],
      ['inOneWeek', '2026-11-07'],
    ]);
  });

  it('propose maintenant (à la minute), dans une heure et demain matin', () => {
    expect(datePresets(true, now).map((p) => [p.label, localDateTimeText(p.date)])).toEqual([
      ['now', '2026-10-31 14:27:00'],
      ['inOneHour', '2026-10-31 15:27:00'],
      ['tomorrowMorning', '2026-11-01 09:00:00'],
    ]);
  });
});
