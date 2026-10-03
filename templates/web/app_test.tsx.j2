// NE PAS MODIFIER : code généré par forge depuis `forge.json`.

import { FakeApi } from '@forge/web/testing';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { App, schema } from './app';

describe('chaque table s\'affiche', () => {
  for (const table of schema.tables) {
    for (const path of [`/data/${table.name}`, `/data/${table.name}/new`]) {
      it(path, async () => {
        const api = new FakeApi();
        for (const t of schema.tables) api.records(t.name, []);
        render(<App client={api.client()} initialPath={path} />);
        expect(await screen.findAllByRole('heading', { level: 2 })).not.toHaveLength(0);
      });
    }
  }
});
