import { createContext, useContext } from 'react';

import type { ForgeClient, ForgeUser } from './api/client';
import type { ForgeCustomization } from './customization';
import type { ForgeStrings } from './i18n';
import type { AppSchema, Operation, TableSchema } from './schema';
import { ADMIN_ROLE, allows } from './schema';
import type { TitleCache } from './titles';
import type { ValueFormat } from './values';

/** Contexte de l'application, accessible par `useForge()` dans toutes les pages. */
export interface Forge {
  schema: AppSchema;
  client: ForgeClient;
  customization: ForgeCustomization;
  format: ValueFormat;
  strings: ForgeStrings;
  titles: TitleCache;
  user: ForgeUser | null;
  locale: string;
  setLocale: (locale: string) => void;
  /**
   * Version des données, incrémentée après chaque écriture : les pages
   * ouvertes se rechargent (une écriture peut changer des formules d'autres
   * tables).
   */
  dataVersion: number;
  notifyChange: () => void;
}

export const ForgeContext = createContext<Forge | null>(null);

export function useForge(): Forge {
  const forge = useContext(ForgeContext);
  if (!forge) throw new Error('useForge appelé hors de ForgeApp');
  return forge;
}

export const rolesOf = (forge: Forge) => forge.user?.roles ?? [];

export const isAdmin = (forge: Forge) => rolesOf(forge).includes(ADMIN_ROLE);

export const can = (forge: Forge, table: TableSchema, operation: Operation) =>
  allows(table, rolesOf(forge), operation);
