import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:forge_flutter/testing.dart';

import 'support.dart';

/// Une table par modèle de champ, et une table cible de référence.
const models = AppSchema(
  name: 'modeles',
  defaultLocale: 'fr',
  locales: ['fr'],
  roles: ['admin'],
  tables: [
    TableSchema(
      'fiche',
      columns: [
        ColumnSchema(
          'nom',
          ColumnType.string,
          required: true,
          titleField: true,
        ),
        ColumnSchema('couleur', ColumnType.color),
        ColumnSchema('courriel', ColumnType.email),
        ColumnSchema('note', ColumnType.rating, max: 5),
        ColumnSchema('remise', ColumnType.percent),
        ColumnSchema('budget', ColumnType.money, currency: 'EUR'),
        ColumnSchema('description', ColumnType.markdown),
        ColumnSchema('contrat', ColumnType.file, maxSize: 1),
        ColumnSchema('societe', ColumnType.reference, target: 'societe'),
      ],
    ),
    TableSchema(
      'societe',
      columns: [
        ColumnSchema(
          'nom',
          ColumnType.string,
          required: true,
          titleField: true,
        ),
      ],
    ),
  ],
);

final fiche = {
  'id': 1,
  'nom': 'Acme',
  'couleur': '#3366ff',
  'courriel': 'contact@acme.fr',
  'note': 4,
  'remise': '0.125',
  'budget': '1500',
  'description': 'Client **important**',
  'contrat': {
    'id': '0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a',
    'name': 'contrat.pdf',
    'size': 2048,
    'content_type': 'application/pdf',
    'url': '/api/files/0b9f3c1e?expires=1&signature=x',
  },
  'societe': null,
  'created_at': '2026-10-01T08:00:00Z',
  'updated_at': '2026-10-01T08:00:00Z',
};

FakeApi modelsApi() {
  final api = FakeApi(
    user: {
      'id': 1,
      'email': 'admin@test',
      'roles': ['admin'],
    },
  );
  api.records('fiche', [fiche]);
  api.records('societe', []);
  return api;
}

void main() {
  testWidgets('fiche : couleur, lien, étoiles, %, montant, Markdown, fichier', (
    tester,
  ) async {
    await startApp(tester, api: modelsApi(), appSchema: models);
    await openTable(tester, 'Fiche');
    await tester.tap(find.text('Acme'));
    await tester.pumpAndSettle();

    expect(find.text('#3366ff'), findsOneWidget);
    expect(find.byType(LinkValue), findsOneWidget);
    expect(find.byIcon(Icons.star_rounded), findsNWidgets(4));
    expect(find.textContaining(RegExp(r'^12,5\s%$')), findsOneWidget);
    expect(find.textContaining('€'), findsOneWidget);
    expect(find.byType(MarkdownBody), findsOneWidget);
    expect(find.text('contrat.pdf'), findsOneWidget);
    expect(find.text('2 Ko'), findsOneWidget);
  });

  testWidgets('saisie : couleur, note, pourcentage, e-mail, référence créée', (
    tester,
  ) async {
    final api = modelsApi();
    Map<String, Object?>? posted;
    api.on('POST', '/api/societe', (request, _) {
      return {'id': 7, 'nom': (jsonDecode(request.body) as Map)['nom']};
    });
    api.on('POST', '/api/fiche', (request, _) {
      posted = jsonDecode(request.body) as Map<String, Object?>;
      return fiche;
    });
    await startApp(tester, api: api, appSchema: models);
    await openTable(tester, 'Fiche');
    await tester.tap(find.byTooltip('Nouveau'));
    await tester.pumpAndSettle();

    await tester.enterText(find.widgetWithText(TextField, 'Nom *'), 'Initech');
    // Couleur : nuancier.
    await tester.tap(find.byIcon(Icons.palette_outlined));
    await tester.pumpAndSettle();
    expect(find.text('Choisir une couleur'), findsOneWidget);
    await tester.enterText(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.byType(TextField),
      ),
      'AA00CC',
    );
    await tester.pump();
    await tester.tap(find.text('OK'));
    await tester.pumpAndSettle();
    expect(find.text('#aa00cc'), findsOneWidget);
    // Note : troisième étoile.
    await tester.tap(find.byIcon(Icons.star_outline_rounded).at(2));
    await tester.pump();
    await tester.enterText(find.widgetWithText(TextField, 'Remise'), '12,5');
    await tester.enterText(find.widgetWithText(TextField, 'Courriel'), 'non');

    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(find.text('Valeur invalide'), findsOneWidget);
    expect(posted, isNull);
    await tester.enterText(
      find.widgetWithText(TextField, 'Courriel'),
      'it@initech.com',
    );

    // Référence : création depuis le champ, qui revient avec l'identifiant.
    api.records('societe', [
      {'id': 7, 'nom': 'Initech SA'},
    ]);
    await tester.tap(find.byTooltip('Nouveau : Societe'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextField, 'Nom *'),
      'Initech SA',
    );
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(find.text('Initech SA'), findsOneWidget);

    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(posted, {
      'nom': 'Initech',
      'couleur': '#aa00cc',
      'courriel': 'it@initech.com',
      'note': 3,
      'remise': '0.125',
      'societe': 7,
    });
  });
}
