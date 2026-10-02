import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:forge_flutter/testing.dart';

import 'support.dart';

void main() {
  testWidgets('connexion puis liste de la première table lisible', (
    tester,
  ) async {
    await startApp(tester, signedIn: false);
    expect(find.text('Se connecter'), findsOneWidget);

    await tester.enterText(find.byType(TextFormField).at(0), 'admin@test');
    await tester.enterText(find.byType(TextFormField).at(1), 'faux');
    await tester.tap(find.text('Se connecter'));
    await tester.pumpAndSettle();
    expect(find.text('E-mail ou mot de passe incorrect'), findsOneWidget);

    await tester.enterText(find.byType(TextFormField).at(1), 'secret');
    await tester.tap(find.text('Se connecter'));
    await tester.pumpAndSettle();
    expect(find.text('Acme'), findsOneWidget);
    expect(find.text('Globex'), findsOneWidget);
    // Valeurs mises en forme, colonne calculée comprise.
    expect(find.text('Industrie'), findsOneWidget);
    expect(find.text('1 500'), findsOneWidget);
  });

  testWidgets('fiche : valeurs, référence résolue, liste liée', (tester) async {
    final api = await startApp(tester);
    await tester.tap(find.text('Acme'));
    await tester.pumpAndSettle();
    // Les opportunités de l'entreprise, filtrées sur la référence.
    expect(find.text('Contrat cadre'), findsOneWidget);
    expect(
      requestsTo(api, 'GET', '/api/opportunite').last.queryParameters,
      containsPair('entreprise', '1'),
    );

    await tester.tap(find.text('Contrat cadre'));
    await tester.pumpAndSettle();
    expect(find.text('Contrat cadre'), findsWidgets);
    // Référence affichée par l'intitulé de l'entreprise, chargé par lot.
    expect(find.text('Acme'), findsOneWidget);
    expect(find.text('1 500,5'), findsOneWidget);
    expect(find.text('15/10/2026'), findsOneWidget);
    // Colonne masquée absente.
    expect(find.text('Notes'), findsNothing);
  });

  testWidgets('création : validation, erreurs du serveur, enregistrement', (
    tester,
  ) async {
    final api = await startApp(tester);
    Map<String, Object?>? posted;
    var reject = true;
    api.on('POST', '/api/opportunite', (request, _) {
      posted = jsonDecode(request.body) as Map<String, Object?>;
      if (reject) {
        return const FakeResponse(422, {
          'error': {
            'code': 'validation',
            'message': 'données invalides',
            'fields': {
              'titre': ['titre déjà utilisé'],
            },
          },
        });
      }
      return opportunite(12, 'Nouveau contrat');
    });
    await tester.tap(find.text('Opportunité'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Nouveau'));
    await tester.pumpAndSettle();

    // Champs requis vides : refus côté client, sans appel.
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(find.text('Valeur obligatoire'), findsNWidgets(3));
    expect(posted, isNull);

    await tester.enterText(
      find.widgetWithText(TextField, 'Titre *'),
      'Nouveau contrat',
    );
    await tester.enterText(
      find.widgetWithText(TextField, 'Montant *'),
      '2 500,5',
    );
    await tester.tap(
      find.ancestor(
        of: find.text('Entreprise *'),
        matching: find.byType(InputDecorator),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Globex'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(posted, {
      'titre': 'Nouveau contrat',
      'entreprise': 2,
      'montant': '2500.5',
      'etape': 'prospect',
    });
    expect(find.text('titre déjà utilisé'), findsOneWidget);

    reject = false;
    api.on(
      'GET',
      '/api/opportunite/12',
      (_, _) => opportunite(12, 'Nouveau contrat'),
    );
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    // Fiche de l'enregistrement créé.
    expect(find.byTooltip('Modifier'), findsOneWidget);
    expect(find.text('Nouveau contrat'), findsWidgets);
  });

  testWidgets('modification : seules les colonnes changées sont envoyées', (
    tester,
  ) async {
    final api = await startApp(tester);
    Map<String, Object?>? patched;
    api.on('PATCH', r'/api/opportunite/(\d+)', (request, _) {
      patched = jsonDecode(request.body) as Map<String, Object?>;
      return opportunite(10, 'Contrat révisé');
    });
    await tester.tap(find.text('Opportunité'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Contrat cadre'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Modifier'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextField, 'Titre *'),
      'Contrat révisé',
    );
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(patched, {'titre': 'Contrat révisé'});
  });

  testWidgets('filtre sur une énumération, recherche', (tester) async {
    final api = await startApp(tester);
    await tester.tap(find.text('Opportunité'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Filtrer'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Etape').last);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilterChip, 'Gagne'));
    await tester.tap(find.text('Appliquer'));
    await tester.pumpAndSettle();
    expect(find.text('Etape : '), findsOneWidget);
    expect(
      requestsTo(api, 'GET', '/api/opportunite').last.queryParameters,
      containsPair('etape[in]', 'gagne'),
    );

    await tester.enterText(
      find.widgetWithText(TextField, 'Rechercher'),
      'cadre',
    );
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pumpAndSettle();
    expect(
      requestsTo(api, 'GET', '/api/opportunite').last.queryParameters,
      allOf(containsPair('q', 'cadre'), containsPair('etape[in]', 'gagne')),
    );
  });

  testWidgets('calendrier et statistiques', (tester) async {
    final api = fakeApi();
    api.on(
      'GET',
      '/api/opportunite/aggregate',
      (_, _) => {
        'fields': ['montant'],
        'group_by': 'etape',
        'groups': [
          {
            'key': 'prospect',
            'count': 2,
            'montant': {
              'sum': '3001',
              'avg': '1500.5',
              'min': '1500.5',
              'max': '1500.5',
            },
          },
        ],
        'total': {
          'count': 2,
          'montant': {
            'sum': '3001',
            'avg': '1500.5',
            'min': '1500.5',
            'max': '1500.5',
          },
        },
      },
    );
    await startApp(tester, api: api);
    await tester.tap(find.text('Opportunité'));
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.calendar_month));
    await tester.pumpAndSettle();
    final dates = requestsTo(api, 'GET', '/api/opportunite').last;
    expect(
      dates.queryParameters.keys,
      containsAll(['date_cloture[gte]', 'date_cloture[lt]']),
    );

    await tester.tap(find.byIcon(Icons.bar_chart));
    await tester.pumpAndSettle();
    expect(find.text('3 001'), findsWidgets);
    expect(find.text('Prospect'), findsWidgets);
  });

  testWidgets('droits : un lecteur ne voit ni création ni administration', (
    tester,
  ) async {
    await startApp(tester, api: fakeApi(roles: ['lecteur']));
    await tester.tap(find.text('Opportunité'));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Nouveau'), findsNothing);
    expect(find.text('Utilisateurs'), findsNothing);
    expect(find.text('Paramètres'), findsNothing);

    await tester.tap(find.text('Contrat cadre'));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Modifier'), findsNothing);
  });

  testWidgets('paramètres : modification d\'une valeur', (tester) async {
    final api = fakeApi();
    Object? written;
    api.on(
      'GET',
      '/api/parameters',
      (_, _) => [
        {'name': 'tva', 'type': 'decimal', 'label': null, 'value': 20},
      ],
    );
    api.on('PUT', '/api/parameters/tva', (request, _) {
      written = jsonDecode(request.body);
      return {'name': 'tva', 'value': 5.5};
    });
    await startApp(tester, api: api);
    await tester.tap(find.text('Paramètres'));
    await tester.pumpAndSettle();
    expect(find.text('TVA'), findsOneWidget);
    await tester.tap(find.text('TVA'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), '5,5');
    await tester.tap(find.text('Enregistrer'));
    await tester.pumpAndSettle();
    expect(written, {'value': '5.5'});
  });

  testWidgets('petit écran : menu en tiroir, liste en tuiles, langue', (
    tester,
  ) async {
    await startApp(tester, size: const Size(400, 800));
    expect(find.byType(DataTable), findsNothing);
    expect(find.text('Acme'), findsOneWidget);

    await tester.tap(find.byIcon(Icons.menu));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Mon compte'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Français'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('English').last);
    await tester.pumpAndSettle();
    expect(find.text('My account'), findsOneWidget);
  });

  testWidgets('personnalisation : libellés, champ, bloc de fiche, page', (
    tester,
  ) async {
    await startApp(
      tester,
      customization: ForgeCustomization(
        enumLabels: const {
          'entreprise.secteur': {'industrie': Label.plain('Industrie lourde')},
        },
        fields: {
          'entreprise.nom': (context, field) => TextField(
            key: const Key('nom-perso'),
            onChanged: field.onChanged,
          ),
        },
        detailSections: {
          'entreprise': [(context, record) => Text('Bloc de ${record['nom']}')],
        },
        pages: [
          CustomPage(
            path: 'tableau',
            label: const Label.plain('Tableau de bord'),
            icon: Icons.dashboard,
            builder: (context) => const Text('Mon tableau'),
          ),
        ],
      ),
    );
    expect(find.text('Industrie lourde'), findsOneWidget);
    await tester.tap(find.text('Acme'));
    await tester.pumpAndSettle();
    expect(find.text('Bloc de Acme'), findsOneWidget);
    await tester.tap(find.byTooltip('Modifier'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('nom-perso')), findsOneWidget);
    await tester.tap(find.text('Tableau de bord'));
    await tester.pumpAndSettle();
    expect(find.text('Mon tableau'), findsOneWidget);
  });
}
