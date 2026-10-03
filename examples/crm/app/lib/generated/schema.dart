// NE PAS MODIFIER : code généré par forge depuis `forge.json`.
// Ce dossier est réécrit à chaque `forge generate` ; votre code va dans
// `lib/custom/`.

import 'package:forge_flutter/forge_flutter.dart';

/// Description des tables de l'application, lue par l'interface.
const schema = AppSchema(
  name: 'mini_crm',
  defaultLocale: 'fr',
  locales: ['fr', 'en'],
  roles: ['admin', 'commercial', 'lecteur'],
  parameters: [
    Parameter(
      'tva',
      ColumnType.decimal,
      label: Label({'en': 'VAT rate (%)', 'fr': 'Taux de TVA (%)'}),
    ),
    Parameter(
      'devise',
      ColumnType.string,
      label: Label({'en': 'Currency', 'fr': 'Devise'}),
    ),
  ],
  tables: [
    TableSchema(
      'entreprise',
      label: Label({'en': 'Company', 'fr': 'Entreprise'}),
      columns: [
        ColumnSchema(
          'nom',
          ColumnType.string,
          label: Label({'en': 'Name', 'fr': 'Nom'}),
          required: true,
          unique: true,
          titleField: true,
        ),
        ColumnSchema(
          'secteur',
          ColumnType.enumeration,
          label: Label({'en': 'Industry', 'fr': 'Secteur'}),
          values: ['industrie', 'services', 'commerce', 'public'],
        ),
        ColumnSchema(
          'ville',
          ColumnType.string,
          label: Label({'en': 'City', 'fr': 'Ville'}),
        ),
        ColumnSchema(
          'site_web',
          ColumnType.url,
          label: Label({'en': 'Website', 'fr': 'Site web'}),
        ),
        ColumnSchema(
          'logo',
          ColumnType.image,
          label: Label({'en': 'Logo', 'fr': 'Logo'}),
          maxSize: 2,
        ),
        ColumnSchema(
          'satisfaction',
          ColumnType.rating,
          label: Label({'en': 'Satisfaction', 'fr': 'Satisfaction'}),
          max: 5,
        ),
        ColumnSchema(
          'chiffre_affaires',
          ColumnType.money,
          label: Label({'en': 'Revenue', 'fr': 'Chiffre d\'affaires'}),
          currency: 'EUR',
        ),
        ColumnSchema(
          'presentation',
          ColumnType.markdown,
          label: Label({'en': 'Overview', 'fr': 'Présentation'}),
        ),
        ColumnSchema(
          'pipeline',
          ColumnType.money,
          label: Label({'en': 'Pipeline', 'fr': 'Pipeline'}),
          computed: true,
          currency: 'EUR',
        ),
        ColumnSchema(
          'pipeline_pondere',
          ColumnType.money,
          label: Label({'en': 'Weighted pipeline', 'fr': 'Pipeline pondéré'}),
          computed: true,
          stored: false,
          currency: 'EUR',
        ),
        ColumnSchema(
          'nb_contacts',
          ColumnType.integer,
          label: Label({'en': 'Contacts', 'fr': 'Contacts'}),
          computed: true,
          stored: false,
        ),
      ],
      stats: StatsView(
        fields: ['chiffre_affaires', 'pipeline'],
        groupBy: 'secteur',
      ),
      rules: [
        Rule(
          ['admin'],
          {
            Operation.read,
            Operation.create,
            Operation.update,
            Operation.delete,
          },
        ),
        Rule(
          ['commercial'],
          {Operation.read, Operation.create, Operation.update},
        ),
        Rule(['lecteur'], {Operation.read}),
      ],
    ),
    TableSchema(
      'contact',
      label: Label({'en': 'Contact', 'fr': 'Contact'}),
      columns: [
        ColumnSchema(
          'prenom',
          ColumnType.string,
          label: Label({'en': 'First name', 'fr': 'Prénom'}),
          titleField: true,
        ),
        ColumnSchema(
          'nom',
          ColumnType.string,
          label: Label({'en': 'Last name', 'fr': 'Nom'}),
          required: true,
          titleField: true,
        ),
        ColumnSchema(
          'email',
          ColumnType.email,
          label: Label({'en': 'Email', 'fr': 'E-mail'}),
          unique: true,
        ),
        ColumnSchema(
          'telephone',
          ColumnType.phone,
          label: Label({'en': 'Phone', 'fr': 'Téléphone'}),
        ),
        ColumnSchema(
          'photo',
          ColumnType.image,
          label: Label({'en': 'Photo', 'fr': 'Photo'}),
          maxSize: 2,
        ),
        ColumnSchema(
          'poste',
          ColumnType.string,
          label: Label({'en': 'Job title', 'fr': 'Poste'}),
        ),
        ColumnSchema(
          'entreprise',
          ColumnType.reference,
          label: Label({'en': 'Company', 'fr': 'Entreprise'}),
          target: 'entreprise',
        ),
        ColumnSchema(
          'secteur',
          ColumnType.enumeration,
          label: Label({'en': 'Industry', 'fr': 'Secteur'}),
          computed: true,
          stored: false,
          values: ['industrie', 'services', 'commerce', 'public'],
        ),
        ColumnSchema(
          'notes',
          ColumnType.text,
          label: Label({'en': 'Notes', 'fr': 'Notes'}),
          hidden: true,
        ),
      ],
      rules: [
        Rule(
          ['admin'],
          {
            Operation.read,
            Operation.create,
            Operation.update,
            Operation.delete,
          },
        ),
        Rule(['commercial'], {Operation.read, Operation.create}),
        Rule(
          ['commercial'],
          {Operation.update, Operation.delete},
          conditional: true,
        ),
        Rule(['lecteur'], {Operation.read}),
      ],
    ),
    TableSchema(
      'opportunite',
      label: Label({'en': 'Opportunity', 'fr': 'Opportunité'}),
      columns: [
        ColumnSchema(
          'titre',
          ColumnType.string,
          label: Label({'en': 'Title', 'fr': 'Titre'}),
          required: true,
          titleField: true,
        ),
        ColumnSchema(
          'entreprise',
          ColumnType.reference,
          label: Label({'en': 'Company', 'fr': 'Entreprise'}),
          required: true,
          target: 'entreprise',
        ),
        ColumnSchema(
          'contact',
          ColumnType.reference,
          label: Label({'en': 'Main contact', 'fr': 'Contact principal'}),
          target: 'contact',
        ),
        ColumnSchema(
          'secteur',
          ColumnType.enumeration,
          label: Label({'en': 'Industry', 'fr': 'Secteur'}),
          computed: true,
          stored: false,
          values: ['industrie', 'services', 'commerce', 'public'],
        ),
        ColumnSchema(
          'montant',
          ColumnType.money,
          label: Label({'en': 'Amount', 'fr': 'Montant HT'}),
          required: true,
          currency: 'EUR',
        ),
        ColumnSchema(
          'probabilite',
          ColumnType.integer,
          label: Label({'en': 'Probability (%)', 'fr': 'Probabilité (%)'}),
          defaultValue: 50,
        ),
        ColumnSchema(
          'montant_pondere',
          ColumnType.money,
          label: Label({'en': 'Weighted amount', 'fr': 'Montant pondéré'}),
          computed: true,
          currency: 'EUR',
        ),
        ColumnSchema(
          'montant_ttc',
          ColumnType.money,
          label: Label({'en': 'Amount incl. VAT', 'fr': 'Montant TTC'}),
          computed: true,
          currency: 'EUR',
        ),
        ColumnSchema(
          'etape',
          ColumnType.enumeration,
          label: Label({'en': 'Stage', 'fr': 'Étape'}),
          defaultValue: 'prospect',
          values: ['prospect', 'proposition', 'gagne', 'perdu'],
        ),
        ColumnSchema(
          'date_cloture',
          ColumnType.date,
          label: Label({'en': 'Close date', 'fr': 'Date de clôture'}),
        ),
        ColumnSchema(
          'jours_restants',
          ColumnType.integer,
          label: Label({'en': 'Days left', 'fr': 'Jours restants'}),
          computed: true,
          stored: false,
        ),
        ColumnSchema(
          'tags',
          ColumnType.referenceList,
          label: Label({'en': 'Tags', 'fr': 'Étiquettes'}),
          stored: false,
          target: 'tag',
        ),
        ColumnSchema(
          'devis',
          ColumnType.file,
          label: Label({'en': 'Quote', 'fr': 'Devis'}),
          maxSize: 5,
          accept: ['.pdf', '.docx', '.odt'],
        ),
        ColumnSchema(
          'notes_internes',
          ColumnType.text,
          label: Label({'en': 'Internal notes', 'fr': 'Notes internes'}),
          hidden: true,
        ),
      ],
      calendar: CalendarView(start: 'date_cloture'),
      stats: StatsView(
        fields: ['montant', 'montant_pondere'],
        groupBy: 'etape',
      ),
      rules: [
        Rule(
          ['admin'],
          {
            Operation.read,
            Operation.create,
            Operation.update,
            Operation.delete,
          },
        ),
        Rule(['commercial'], {Operation.read, Operation.create}),
        Rule(
          ['commercial'],
          {Operation.update, Operation.delete},
          conditional: true,
        ),
        Rule(['lecteur'], {Operation.read}),
      ],
    ),
    TableSchema(
      'activite',
      label: Label({'en': 'Activity', 'fr': 'Activité'}),
      columns: [
        ColumnSchema(
          'sujet',
          ColumnType.string,
          label: Label({'en': 'Subject', 'fr': 'Sujet'}),
          required: true,
          titleField: true,
        ),
        ColumnSchema(
          'nature',
          ColumnType.enumeration,
          label: Label({'en': 'Kind', 'fr': 'Nature'}),
          required: true,
          values: ['appel', 'reunion', 'email', 'tache'],
        ),
        ColumnSchema(
          'debut',
          ColumnType.datetime,
          label: Label({'en': 'Start', 'fr': 'Début'}),
          required: true,
        ),
        ColumnSchema(
          'duree',
          ColumnType.duration,
          label: Label({'en': 'Duration', 'fr': 'Durée'}),
          defaultValue: 1800,
        ),
        ColumnSchema(
          'opportunite',
          ColumnType.reference,
          label: Label({'en': 'Opportunity', 'fr': 'Opportunité'}),
          target: 'opportunite',
        ),
        ColumnSchema(
          'contact',
          ColumnType.reference,
          label: Label({'en': 'Contact', 'fr': 'Contact'}),
          target: 'contact',
        ),
        ColumnSchema(
          'terminee',
          ColumnType.boolean,
          label: Label({'en': 'Done', 'fr': 'Terminée'}),
          defaultValue: false,
        ),
        ColumnSchema(
          'compte_rendu',
          ColumnType.markdown,
          label: Label({'en': 'Report', 'fr': 'Compte rendu'}),
        ),
      ],
      calendar: CalendarView(start: 'debut', duration: 'duree'),
      stats: StatsView(fields: ['duree'], groupBy: 'nature'),
      rules: [
        Rule(
          ['admin'],
          {
            Operation.read,
            Operation.create,
            Operation.update,
            Operation.delete,
          },
        ),
        Rule(['commercial'], {Operation.read, Operation.create}),
        Rule(
          ['commercial'],
          {Operation.update, Operation.delete},
          conditional: true,
        ),
        Rule(['lecteur'], {Operation.read}),
      ],
    ),
    TableSchema(
      'tag',
      label: Label({'en': 'Tag', 'fr': 'Étiquette'}),
      columns: [
        ColumnSchema(
          'nom',
          ColumnType.string,
          label: Label({'en': 'Name', 'fr': 'Nom'}),
          required: true,
          unique: true,
          titleField: true,
        ),
        ColumnSchema(
          'couleur',
          ColumnType.color,
          label: Label({'en': 'Color', 'fr': 'Couleur'}),
          defaultValue: '#607d8b',
        ),
        ColumnSchema(
          'nb_opportunites',
          ColumnType.integer,
          label: Label({'en': 'Opportunities', 'fr': 'Opportunités'}),
          computed: true,
          stored: false,
        ),
      ],
      rules: [
        Rule(
          ['admin'],
          {
            Operation.read,
            Operation.create,
            Operation.update,
            Operation.delete,
          },
        ),
        Rule(['commercial', 'lecteur'], {Operation.read}),
      ],
    ),
  ],
);
