// NE PAS MODIFIER : code généré par forge depuis `forge.json`.
// Ce dossier est réécrit à chaque `forge generate` ; votre code va dans
// `src/custom/`.

import type { AppSchema } from '@forge/web';

/** Description des tables de l'application, lue par l'interface. */
export const schema: AppSchema = {
  "default_locale": "fr",
  "locales": [
    "fr",
    "en"
  ],
  "name": "mini_crm",
  "parameters": [
    {
      "label": {
        "en": "VAT rate (%)",
        "fr": "Taux de TVA (%)"
      },
      "name": "tva",
      "type": "decimal"
    },
    {
      "label": {
        "en": "Currency",
        "fr": "Devise"
      },
      "name": "devise",
      "type": "string"
    }
  ],
  "roles": [
    "admin",
    "commercial",
    "lecteur"
  ],
  "tables": [
    {
      "columns": [
        {
          "label": {
            "en": "Name",
            "fr": "Nom"
          },
          "name": "nom",
          "required": true,
          "title_field": true,
          "type": "string",
          "unique": true
        },
        {
          "label": {
            "en": "Industry",
            "fr": "Secteur"
          },
          "name": "secteur",
          "type": "enum",
          "values": [
            "industrie",
            "services",
            "commerce",
            "public"
          ]
        },
        {
          "label": {
            "en": "City",
            "fr": "Ville"
          },
          "name": "ville",
          "type": "string"
        },
        {
          "label": {
            "en": "Website",
            "fr": "Site web"
          },
          "name": "site_web",
          "type": "string"
        },
        {
          "label": {
            "en": "Revenue",
            "fr": "Chiffre d'affaires"
          },
          "name": "chiffre_affaires",
          "type": "decimal"
        },
        {
          "computed": true,
          "label": {
            "en": "Pipeline",
            "fr": "Pipeline"
          },
          "name": "pipeline",
          "type": "decimal"
        },
        {
          "computed": true,
          "label": {
            "en": "Weighted pipeline",
            "fr": "Pipeline pondéré"
          },
          "name": "pipeline_pondere",
          "type": "decimal",
          "virtual": true
        },
        {
          "computed": true,
          "label": {
            "en": "Contacts",
            "fr": "Contacts"
          },
          "name": "nb_contacts",
          "type": "integer",
          "virtual": true
        }
      ],
      "label": {
        "en": "Company",
        "fr": "Entreprise"
      },
      "name": "entreprise",
      "rules": [
        {
          "operations": [
            "read",
            "create",
            "update",
            "delete"
          ],
          "roles": [
            "admin"
          ]
        },
        {
          "operations": [
            "read",
            "create",
            "update"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "operations": [
            "read"
          ],
          "roles": [
            "lecteur"
          ]
        }
      ],
      "stats": {
        "fields": [
          "chiffre_affaires",
          "pipeline"
        ],
        "group_by": "secteur"
      }
    },
    {
      "columns": [
        {
          "label": {
            "en": "First name",
            "fr": "Prénom"
          },
          "name": "prenom",
          "title_field": true,
          "type": "string"
        },
        {
          "label": {
            "en": "Last name",
            "fr": "Nom"
          },
          "name": "nom",
          "required": true,
          "title_field": true,
          "type": "string"
        },
        {
          "label": {
            "en": "Email",
            "fr": "E-mail"
          },
          "name": "email",
          "type": "string",
          "unique": true
        },
        {
          "label": {
            "en": "Phone",
            "fr": "Téléphone"
          },
          "name": "telephone",
          "type": "string"
        },
        {
          "label": {
            "en": "Job title",
            "fr": "Poste"
          },
          "name": "poste",
          "type": "string"
        },
        {
          "label": {
            "en": "Company",
            "fr": "Entreprise"
          },
          "name": "entreprise",
          "target": "entreprise",
          "type": "reference"
        },
        {
          "computed": true,
          "label": {
            "en": "Industry",
            "fr": "Secteur"
          },
          "name": "secteur",
          "type": "enum",
          "values": [
            "industrie",
            "services",
            "commerce",
            "public"
          ],
          "virtual": true
        },
        {
          "hidden": true,
          "label": {
            "en": "Notes",
            "fr": "Notes"
          },
          "name": "notes",
          "type": "text"
        }
      ],
      "label": {
        "en": "Contact",
        "fr": "Contact"
      },
      "name": "contact",
      "rules": [
        {
          "operations": [
            "read",
            "create",
            "update",
            "delete"
          ],
          "roles": [
            "admin"
          ]
        },
        {
          "operations": [
            "read",
            "create"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "conditional": true,
          "operations": [
            "update",
            "delete"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "operations": [
            "read"
          ],
          "roles": [
            "lecteur"
          ]
        }
      ]
    },
    {
      "calendar": {
        "start": "date_cloture"
      },
      "columns": [
        {
          "label": {
            "en": "Title",
            "fr": "Titre"
          },
          "name": "titre",
          "required": true,
          "title_field": true,
          "type": "string"
        },
        {
          "label": {
            "en": "Company",
            "fr": "Entreprise"
          },
          "name": "entreprise",
          "required": true,
          "target": "entreprise",
          "type": "reference"
        },
        {
          "label": {
            "en": "Main contact",
            "fr": "Contact principal"
          },
          "name": "contact",
          "target": "contact",
          "type": "reference"
        },
        {
          "computed": true,
          "label": {
            "en": "Industry",
            "fr": "Secteur"
          },
          "name": "secteur",
          "type": "enum",
          "values": [
            "industrie",
            "services",
            "commerce",
            "public"
          ],
          "virtual": true
        },
        {
          "label": {
            "en": "Amount",
            "fr": "Montant HT"
          },
          "name": "montant",
          "required": true,
          "type": "decimal"
        },
        {
          "default": 50,
          "label": {
            "en": "Probability (%)",
            "fr": "Probabilité (%)"
          },
          "name": "probabilite",
          "type": "integer"
        },
        {
          "computed": true,
          "label": {
            "en": "Weighted amount",
            "fr": "Montant pondéré"
          },
          "name": "montant_pondere",
          "type": "decimal"
        },
        {
          "computed": true,
          "label": {
            "en": "Amount incl. VAT",
            "fr": "Montant TTC"
          },
          "name": "montant_ttc",
          "type": "decimal"
        },
        {
          "default": "prospect",
          "label": {
            "en": "Stage",
            "fr": "Étape"
          },
          "name": "etape",
          "type": "enum",
          "values": [
            "prospect",
            "proposition",
            "gagne",
            "perdu"
          ]
        },
        {
          "label": {
            "en": "Close date",
            "fr": "Date de clôture"
          },
          "name": "date_cloture",
          "type": "date"
        },
        {
          "computed": true,
          "label": {
            "en": "Days left",
            "fr": "Jours restants"
          },
          "name": "jours_restants",
          "type": "integer",
          "virtual": true
        },
        {
          "label": {
            "en": "Tags",
            "fr": "Étiquettes"
          },
          "name": "tags",
          "target": "tag",
          "type": "reference_list",
          "virtual": true
        },
        {
          "hidden": true,
          "label": {
            "en": "Internal notes",
            "fr": "Notes internes"
          },
          "name": "notes_internes",
          "type": "text"
        }
      ],
      "label": {
        "en": "Opportunity",
        "fr": "Opportunité"
      },
      "name": "opportunite",
      "rules": [
        {
          "operations": [
            "read",
            "create",
            "update",
            "delete"
          ],
          "roles": [
            "admin"
          ]
        },
        {
          "operations": [
            "read",
            "create"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "conditional": true,
          "operations": [
            "update",
            "delete"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "operations": [
            "read"
          ],
          "roles": [
            "lecteur"
          ]
        }
      ],
      "stats": {
        "fields": [
          "montant",
          "montant_pondere"
        ],
        "group_by": "etape"
      }
    },
    {
      "calendar": {
        "duration": "duree",
        "start": "debut"
      },
      "columns": [
        {
          "label": {
            "en": "Subject",
            "fr": "Sujet"
          },
          "name": "sujet",
          "required": true,
          "title_field": true,
          "type": "string"
        },
        {
          "label": {
            "en": "Kind",
            "fr": "Nature"
          },
          "name": "nature",
          "required": true,
          "type": "enum",
          "values": [
            "appel",
            "reunion",
            "email",
            "tache"
          ]
        },
        {
          "label": {
            "en": "Start",
            "fr": "Début"
          },
          "name": "debut",
          "required": true,
          "type": "datetime"
        },
        {
          "default": 1800,
          "label": {
            "en": "Duration",
            "fr": "Durée"
          },
          "name": "duree",
          "type": "duration"
        },
        {
          "label": {
            "en": "Opportunity",
            "fr": "Opportunité"
          },
          "name": "opportunite",
          "target": "opportunite",
          "type": "reference"
        },
        {
          "label": {
            "en": "Contact",
            "fr": "Contact"
          },
          "name": "contact",
          "target": "contact",
          "type": "reference"
        },
        {
          "default": false,
          "label": {
            "en": "Done",
            "fr": "Terminée"
          },
          "name": "terminee",
          "type": "boolean"
        },
        {
          "label": {
            "en": "Report",
            "fr": "Compte rendu"
          },
          "name": "compte_rendu",
          "type": "text"
        }
      ],
      "label": {
        "en": "Activity",
        "fr": "Activité"
      },
      "name": "activite",
      "rules": [
        {
          "operations": [
            "read",
            "create",
            "update",
            "delete"
          ],
          "roles": [
            "admin"
          ]
        },
        {
          "operations": [
            "read",
            "create"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "conditional": true,
          "operations": [
            "update",
            "delete"
          ],
          "roles": [
            "commercial"
          ]
        },
        {
          "operations": [
            "read"
          ],
          "roles": [
            "lecteur"
          ]
        }
      ],
      "stats": {
        "fields": [
          "duree"
        ],
        "group_by": "nature"
      }
    },
    {
      "columns": [
        {
          "label": {
            "en": "Name",
            "fr": "Nom"
          },
          "name": "nom",
          "required": true,
          "title_field": true,
          "type": "string",
          "unique": true
        },
        {
          "default": "#607D8B",
          "label": {
            "en": "Color",
            "fr": "Couleur"
          },
          "name": "couleur",
          "type": "string"
        },
        {
          "computed": true,
          "label": {
            "en": "Opportunities",
            "fr": "Opportunités"
          },
          "name": "nb_opportunites",
          "type": "integer",
          "virtual": true
        }
      ],
      "label": {
        "en": "Tag",
        "fr": "Étiquette"
      },
      "name": "tag",
      "rules": [
        {
          "operations": [
            "read",
            "create",
            "update",
            "delete"
          ],
          "roles": [
            "admin"
          ]
        },
        {
          "operations": [
            "read"
          ],
          "roles": [
            "commercial",
            "lecteur"
          ]
        }
      ]
    }
  ]
};
