import type { MantineThemeOverride } from '@mantine/core';
import type { Icon } from '@tabler/icons-react';
import type { ComponentType } from 'react';

import type { Json } from './api/client';
import type { ForgeStrings } from './i18n';
import type { ColumnSchema, Label, TableSchema } from './schema';
import type { EnumLabels } from './values';

/** Champ de formulaire : valeur au format JSON de l'API et erreur à afficher. */
export interface FieldProps {
  table: TableSchema;
  column: ColumnSchema;
  value: unknown;
  onChange: (value: unknown) => void;
  error?: string;
}

/** Affichage d'une valeur (liste, fiche). */
export interface CellProps {
  table: TableSchema;
  column: ColumnSchema;
  record: Json;
  value: unknown;
}

/** Bloc ajouté à la fiche d'un enregistrement. */
export interface DetailSectionProps {
  table: TableSchema;
  record: Json;
}

/** Page ajoutée au menu, servie sous `/pages/<path>`. */
export interface CustomPage {
  path: string;
  label: Label;
  icon: Icon;
  component: ComponentType;
  /** Rôles qui voient la page (tous si absent). */
  roles?: string[];
}

/**
 * Points d'extension de l'application générée, déclarés dans
 * `src/custom/customization.tsx` (jamais écrasé par `forge generate`).
 * Les clés `table.colonne` désignent une colonne.
 */
export interface ForgeCustomization {
  /** Thème Mantine fusionné avec celui de forge (couleur principale, police…). */
  theme?: MantineThemeOverride;
  /** Icône d'une table dans le menu et l'accueil. */
  tableIcons?: Record<string, Icon>;
  /** Libellés des valeurs d'énumération (`table.colonne` → valeur → libellé). */
  enumLabels?: EnumLabels;
  /** Champs de formulaire remplaçant ceux par défaut (`table.colonne`). */
  fields?: Record<string, ComponentType<FieldProps>>;
  /** Affichages de valeurs remplaçant ceux par défaut (`table.colonne`). */
  cells?: Record<string, ComponentType<CellProps>>;
  /** Blocs supplémentaires de la fiche, par table. */
  detailSections?: Record<string, ComponentType<DetailSectionProps>[]>;
  pages?: CustomPage[];
  /** Textes de l'interface par langue, en plus ou à la place de `fr` et `en`. */
  strings?: Record<string, ForgeStrings>;
}
