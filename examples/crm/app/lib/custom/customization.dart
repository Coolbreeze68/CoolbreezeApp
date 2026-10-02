import 'package:flutter/material.dart';
import 'package:forge_flutter/forge_flutter.dart';

import 'entreprise_summary.dart';
import 'theme.dart';

/// Personnalisation de l'application, jamais écrasée par `forge generate`.
///
/// Points d'extension de `ForgeCustomization` : `tableIcons` (icônes du menu),
/// `enumLabels` (libellés des valeurs d'énumération), `fields` et `cells`
/// (champs de formulaire et affichages par colonne `table.colonne`),
/// `detailSections` (blocs des fiches), `pages` (pages ajoutées au menu) et
/// `strings` (textes de l'interface). Les modèles typés de
/// `lib/generated/models.dart` facilitent l'accès à l'API.
final customization = ForgeCustomization(
  theme: lightTheme,
  darkTheme: darkTheme,
  tableIcons: const {
    'entreprise': Icons.business,
    'contact': Icons.person_outline,
    'opportunite': Icons.trending_up,
    'activite': Icons.event_note,
    'tag': Icons.label_outline,
  },
  enumLabels: const {
    'opportunite.etape': {
      'prospect': Label({'fr': 'Prospect', 'en': 'Lead'}),
      'proposition': Label({'fr': 'Proposition', 'en': 'Proposal'}),
      'gagne': Label({'fr': 'Gagnée', 'en': 'Won'}),
      'perdu': Label({'fr': 'Perdue', 'en': 'Lost'}),
    },
    'activite.nature': {
      'appel': Label({'fr': 'Appel', 'en': 'Call'}),
      'reunion': Label({'fr': 'Réunion', 'en': 'Meeting'}),
      'email': Label({'fr': 'E-mail', 'en': 'Email'}),
      'tache': Label({'fr': 'Tâche', 'en': 'Task'}),
    },
  },
  detailSections: {
    'entreprise': [
      (context, record) => EntrepriseSummary(id: record['id'] as int),
    ],
  },
);
