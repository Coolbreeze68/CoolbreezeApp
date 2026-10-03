// NE PAS MODIFIER : code généré par forge depuis `forge.json`.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:forge_flutter/testing.dart';
import 'package:mini_crm_app/generated/app.dart';
import 'package:mini_crm_app/generated/models.dart';

/// Colonnes modifiables de `table`, avec leur valeur dans `json`.
Map<String, dynamic> writable(String table, Map<String, dynamic> json) => {
  for (final column in schema.table(table)!.columns)
    if (column.writable) column.name: written(column, json[column.name]),
};

/// Valeur envoyée à l'API : un fichier s'écrit par son identifiant.
Object? written(ColumnSchema column, Object? value) =>
    column.type.isFile && value is Map ? value['id'] : value;

void main() {
  testWidgets('chaque table : liste et formulaire de création', (tester) async {
    final api = FakeApi();
    for (final table in schema.tables) {
      api.records(table.name, const []);
    }
    await tester.pumpWidget(buildApp(client: api.client()));
    await tester.pumpAndSettle();
    final router = GoRouter.of(tester.element(find.byType(Navigator).first));
    for (final table in schema.tables) {
      router.go(Paths.table(table.name));
      await tester.pumpAndSettle();
      router.go(Paths.create(table.name));
      await tester.pumpAndSettle();
    }
  });

  test('modèle Entreprise : lecture et écriture', () {
    final json = {
      'id': 1,
      'nom': 'texte',
      'secteur': 'industrie',
      'ville': 'texte',
      'site_web': 'texte',
      'logo': {
        'id': '0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a',
        'name': 'fichier.png',
        'size': 1024,
        'content_type': 'image/png',
        'url': '/api/files/0b9f3c1e',
      },
      'satisfaction': 2,
      'chiffre_affaires': '1234.5',
      'presentation': 'texte',
      'pipeline': '1234.5',
      'pipeline_pondere': '1234.5',
      'nb_contacts': 2,
      'owner': 1,
      'created_at': '2026-01-02T03:04:05.000Z',
      'updated_at': '2026-01-02T03:04:05.000Z',
    };
    expect(
      Entreprise.fromJson(json).toJson(),
      writable('entreprise', json),
    );
  });
  test('modèle Contact : lecture et écriture', () {
    final json = {
      'id': 1,
      'prenom': 'texte',
      'nom': 'texte',
      'email': 'texte',
      'telephone': 'texte',
      'photo': {
        'id': '0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a',
        'name': 'fichier.png',
        'size': 1024,
        'content_type': 'image/png',
        'url': '/api/files/0b9f3c1e',
      },
      'poste': 'texte',
      'entreprise': 2,
      'secteur': 'industrie',
      'notes': 'texte',
      'owner': 1,
      'created_at': '2026-01-02T03:04:05.000Z',
      'updated_at': '2026-01-02T03:04:05.000Z',
    };
    expect(
      Contact.fromJson(json).toJson(),
      writable('contact', json),
    );
  });
  test('modèle Opportunite : lecture et écriture', () {
    final json = {
      'id': 1,
      'titre': 'texte',
      'entreprise': 2,
      'contact': 2,
      'secteur': 'industrie',
      'montant': '1234.5',
      'probabilite': 2,
      'montant_pondere': '1234.5',
      'montant_ttc': '1234.5',
      'etape': 'prospect',
      'date_cloture': '2026-01-02',
      'jours_restants': 2,
      'tags': [2, 3],
      'devis': {
        'id': '0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a',
        'name': 'fichier.png',
        'size': 1024,
        'content_type': 'image/png',
        'url': '/api/files/0b9f3c1e',
      },
      'notes_internes': 'texte',
      'owner': 1,
      'created_at': '2026-01-02T03:04:05.000Z',
      'updated_at': '2026-01-02T03:04:05.000Z',
    };
    expect(
      Opportunite.fromJson(json).toJson(),
      writable('opportunite', json),
    );
  });
  test('modèle Activite : lecture et écriture', () {
    final json = {
      'id': 1,
      'sujet': 'texte',
      'nature': 'appel',
      'debut': '2026-01-02T03:04:05.000Z',
      'duree': 5400,
      'opportunite': 2,
      'contact': 2,
      'terminee': true,
      'compte_rendu': 'texte',
      'owner': 1,
      'created_at': '2026-01-02T03:04:05.000Z',
      'updated_at': '2026-01-02T03:04:05.000Z',
    };
    expect(
      Activite.fromJson(json).toJson(),
      writable('activite', json),
    );
  });
  test('modèle Tag : lecture et écriture', () {
    final json = {
      'id': 1,
      'nom': 'texte',
      'couleur': 'texte',
      'nb_opportunites': 2,
      'owner': 1,
      'created_at': '2026-01-02T03:04:05.000Z',
      'updated_at': '2026-01-02T03:04:05.000Z',
    };
    expect(
      Tag.fromJson(json).toJson(),
      writable('tag', json),
    );
  });
}
