import type { ForgeCustomization } from '@forge/web';
import { IconBuilding, IconCalendarEvent, IconTag, IconTrendingUp, IconUser } from '@tabler/icons-react';

import { EntrepriseSummary } from './EntrepriseSummary';

/**
 * Personnalisation de l'application, jamais écrasée par `forge generate`.
 *
 * Points d'extension de `ForgeCustomization` : `theme` (thème Mantine :
 * couleur principale, police…), `tableIcons` (icônes du menu), `enumLabels`
 * (libellés des valeurs d'énumération), `fields` et `cells` (champs de
 * formulaire et affichages par colonne `table.colonne`), `detailSections`
 * (blocs des fiches), `pages` (pages ajoutées au menu) et `strings` (textes de
 * l'interface). Les modèles typés de `src/generated/models.ts` facilitent
 * l'accès à l'API.
 */
export const customization: ForgeCustomization = {
  theme: { primaryColor: 'indigo' },
  tableIcons: {
    entreprise: IconBuilding,
    contact: IconUser,
    opportunite: IconTrendingUp,
    activite: IconCalendarEvent,
    tag: IconTag,
  },
  enumLabels: {
    'opportunite.etape': {
      prospect: { fr: 'Prospect', en: 'Lead' },
      proposition: { fr: 'Proposition', en: 'Proposal' },
      gagne: { fr: 'Gagnée', en: 'Won' },
      perdu: { fr: 'Perdue', en: 'Lost' },
    },
    'activite.nature': {
      appel: { fr: 'Appel', en: 'Call' },
      reunion: { fr: 'Réunion', en: 'Meeting' },
      email: { fr: 'E-mail', en: 'Email' },
      tache: { fr: 'Tâche', en: 'Task' },
    },
  },
  detailSections: { entreprise: [EntrepriseSummary] },
};
