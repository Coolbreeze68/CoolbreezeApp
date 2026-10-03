/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Adresse de l'API en développement : `npm run dev` lui relaie `/api`
// (même origine pour le navigateur, sans CORS).
const api = process.env.FORGE_API_URL ?? 'http://localhost:8080';

export default defineConfig({
  plugins: [react()],
  resolve: {
    // `@forge/web` est lié depuis les sources de forge : ses imports se
    // résolvent dans les dépendances de cette application.
    preserveSymlinks: true,
    dedupe: ['react', 'react-dom', 'react-router', '@mantine/core', '@mantine/hooks'],
  },
  server: {
    proxy: { '/api': api, '/health': api },
  },
  // Application de gestion chargée une fois (React, Mantine) : un seul paquet.
  build: { chunkSizeWarningLimit: 1500 },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/generated/test-setup.ts'],
    server: { deps: { inline: ['@forge/web'] } },
  },
});
