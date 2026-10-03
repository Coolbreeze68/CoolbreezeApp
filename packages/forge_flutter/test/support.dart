import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:forge_flutter/testing.dart';
import 'package:forge_flutter/src/ui/shell.dart';

/// Schéma de test, sur le modèle du CRM d'exemple.
const schema = AppSchema(
  name: 'crm_test',
  defaultLocale: 'fr',
  locales: ['fr', 'en'],
  roles: ['admin', 'commercial', 'lecteur'],
  parameters: [
    Parameter(
      'tva',
      ColumnType.decimal,
      label: Label({'fr': 'TVA', 'en': 'VAT'}),
    ),
  ],
  tables: [
    TableSchema(
      'entreprise',
      label: Label({'fr': 'Entreprise', 'en': 'Company'}),
      columns: [
        ColumnSchema(
          'nom',
          ColumnType.string,
          label: Label({'fr': 'Nom', 'en': 'Name'}),
          required: true,
          titleField: true,
        ),
        ColumnSchema(
          'secteur',
          ColumnType.enumeration,
          values: ['industrie', 'services'],
        ),
        ColumnSchema('pipeline', ColumnType.decimal, computed: true),
      ],
      rules: [
        Rule(['commercial', 'lecteur'], {Operation.read}),
      ],
    ),
    TableSchema(
      'opportunite',
      label: Label({'fr': 'Opportunité', 'en': 'Opportunity'}),
      columns: [
        ColumnSchema(
          'titre',
          ColumnType.string,
          required: true,
          titleField: true,
        ),
        ColumnSchema(
          'entreprise',
          ColumnType.reference,
          required: true,
          target: 'entreprise',
        ),
        ColumnSchema('montant', ColumnType.decimal, required: true),
        ColumnSchema(
          'etape',
          ColumnType.enumeration,
          values: ['prospect', 'gagne'],
          defaultValue: 'prospect',
        ),
        ColumnSchema('date_cloture', ColumnType.date),
        ColumnSchema('notes', ColumnType.text, hidden: true),
      ],
      calendar: CalendarView(start: 'date_cloture'),
      stats: StatsView(fields: ['montant'], groupBy: 'etape'),
      rules: [
        Rule(['commercial'], {Operation.read, Operation.create}),
        Rule(
          ['commercial'],
          {Operation.update, Operation.delete},
          conditional: true,
        ),
        Rule(['lecteur'], {Operation.read}),
      ],
    ),
  ],
);

final entreprises = [
  {'id': 1, 'nom': 'Acme', 'secteur': 'industrie', 'pipeline': '1500'},
  {'id': 2, 'nom': 'Globex', 'secteur': 'services', 'pipeline': null},
];

Map<String, Object?> opportunite(int id, String titre, {String? date}) => {
  'id': id,
  'titre': titre,
  'entreprise': 1,
  'montant': '1500.5',
  'etape': 'prospect',
  'date_cloture': date,
  'notes': null,
  'created_at': '2026-10-01T08:00:00Z',
  'updated_at': '2026-10-01T08:00:00Z',
};

/// API simulée avec les données de test.
FakeApi fakeApi({List<String> roles = const ['admin']}) {
  final api = FakeApi(user: {'id': 1, 'email': 'admin@test', 'roles': roles});
  api.records('entreprise', entreprises);
  api.records('opportunite', [
    opportunite(10, 'Contrat cadre', date: '2026-10-15'),
    opportunite(11, 'Extension'),
  ]);
  return api;
}

/// Lance l'application, connectée, sur un écran de la taille donnée.
Future<FakeApi> startApp(
  WidgetTester tester, {
  FakeApi? api,
  bool signedIn = true,
  Size size = const Size(1280, 900),
  ForgeCustomization customization = const ForgeCustomization(),
  AppSchema appSchema = schema,
}) async {
  api ??= fakeApi();
  tester.view
    ..devicePixelRatio = 1
    ..physicalSize = size;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    ForgeApp(
      schema: appSchema,
      client: api.client(signedIn: signedIn),
      customization: customization,
    ),
  );
  await tester.pumpAndSettle();
  return api;
}

/// Requêtes reçues pour `method` et `path`.
Iterable<Uri> requestsTo(FakeApi api, String method, String path) => api
    .requests
    .where((r) => r.method == method && r.url.path == path)
    .map((r) => r.url);

/// Ouvre une table par son entrée du menu latéral (le libellé figure aussi
/// sur l'accueil).
Future<void> openTable(WidgetTester tester, String label) async {
  await tester.tap(
    find.descendant(of: find.byType(NavMenu), matching: find.text(label)),
  );
  await tester.pumpAndSettle();
}
