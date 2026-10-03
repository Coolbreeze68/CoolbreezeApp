import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:forge_flutter/testing.dart';

void main() {
  test('connexion, session enregistrée puis reprise', () async {
    final api = FakeApi();
    final client = api.client(signedIn: false);
    expect(await client.restore(), isFalse);

    await expectLater(
      client.signIn('admin@test', 'faux'),
      throwsA(isA<ApiException>().having((e) => e.status, 'status', 401)),
    );
    final user = await client.signIn('admin@test', 'secret');
    expect(user.roles, ['admin']);
    expect(client.signedIn, isTrue);
    expect(await client.store.read(ForgeClient.refreshTokenKey), 'refresh');

    final restored = ForgeClient(
      baseUrl: FakeApi.baseUrl,
      httpClient: api.httpClient,
      store: client.store,
    );
    expect(await restored.restore(), isTrue);
    expect(restored.user!.email, 'admin@test');

    await restored.signOut();
    expect(restored.signedIn, isFalse);
    expect(await client.store.read(ForgeClient.refreshTokenKey), isNull);
  });

  test('jeton expiré : renouvelé une fois, requête rejouée', () async {
    final api = FakeApi();
    var calls = 0;
    api.on('GET', '/api/tag', (request, _) {
      calls++;
      return calls == 1
          ? const FakeResponse(401, {
              'error': {'code': 'unauthorized', 'message': 'expiré'},
            })
          : {'data': [], 'page': 1, 'per_page': 25, 'total': 0};
    });
    final client = api.client();
    await client.restore();
    final listing = await client.table('tag').list();
    expect(listing.total, 0);
    expect(calls, 2);
    final refreshes = api.requests.where(
      (r) => r.url.path == '/api/auth/refresh',
    );
    expect(refreshes, hasLength(2), reason: 'reprise puis renouvellement');
    expect(api.requests.last.headers['authorization'], 'Bearer access');
  });

  test('session refusée au renouvellement : déconnexion', () async {
    final api = FakeApi();
    api.on('POST', '/api/auth/refresh', (_, _) => const FakeResponse(401));
    final client = api.client();
    expect(await client.restore(), isFalse);
    expect(client.signedIn, isFalse);
    expect(await client.store.read(ForgeClient.refreshTokenKey), isNull);
  });

  test('erreurs de validation et d\'import décodées', () async {
    final api = FakeApi();
    api.on(
      'POST',
      '/api/contact',
      (_, _) => const FakeResponse(422, {
        'error': {
          'code': 'validation',
          'message': 'données invalides',
          'fields': {
            'nom': ['valeur obligatoire'],
          },
        },
      }),
    );
    api.on(
      'POST',
      '/api/contact/import',
      (_, _) => const FakeResponse(422, {
        'error': {
          'code': 'import',
          'message': 'import refusé',
          'lines': [
            {
              'line': 3,
              'code': 'validation',
              'message': 'données invalides',
              'fields': {
                'email': ['déjà utilisé'],
              },
            },
          ],
        },
      }),
    );
    final client = api.client();
    await client.restore();
    final contacts = client.table('contact');
    await expectLater(
      contacts.create({}),
      throwsA(
        isA<ApiException>().having((e) => e.code, 'code', 'validation').having(
          (e) => e.fields['nom'],
          'fields',
          ['valeur obligatoire'],
        ),
      ),
    );
    await expectLater(
      contacts.import(Uint8List.fromList(utf8.encode('nom\n'))),
      throwsA(
        isA<ApiException>().having((e) => e.lines.single.line, 'line', 3),
      ),
    );
    expect(api.requests.last.headers['content-type'], 'text/csv');
  });

  test('agrégats : décimaux en texte, groupes et total', () async {
    final api = FakeApi();
    api.on(
      'GET',
      '/api/opportunite/aggregate',
      (_, _) => {
        'fields': ['montant'],
        'group_by': 'etape',
        'groups': [
          {
            'key': 'gagne',
            'count': 2,
            'montant': {
              'sum': '3000',
              'avg': '1500',
              'min': '1000',
              'max': '2000',
            },
          },
        ],
        'total': {
          'count': 2,
          'montant': {
            'sum': '3000',
            'avg': '1500',
            'min': '1000',
            'max': '2000',
          },
        },
      },
    );
    final client = api.client();
    await client.restore();
    final result = await client
        .table('opportunite')
        .aggregate(
          ['montant'],
          groupBy: 'etape',
          query: const ListQuery(filters: [Filter.equals('etape', 'gagne')]),
        );
    expect(result.groups.single.key, 'gagne');
    expect(result.total.measures['montant']!.sum, 3000);
    expect(api.requests.last.url.queryParameters, {
      'etape': 'gagne',
      'fields': 'montant',
      'group_by': 'etape',
    });
  });

  test(
    'stockage indisponible : la session fonctionne sans être retenue',
    () async {
      final api = FakeApi();
      final client = ForgeClient(
        baseUrl: FakeApi.baseUrl,
        httpClient: api.httpClient,
        store: _BrokenStore(),
      );
      var notified = 0;
      client.addListener(() => notified++);
      expect(await client.restore(), isFalse);
      final user = await client.signIn('admin@test', 'secret');
      expect(user.email, 'admin@test');
      expect(client.signedIn, isTrue);
      expect(notified, 1);
      await client.signOut();
      expect(client.signedIn, isFalse);
    },
  );

  test('serveur injoignable : erreur réseau', () async {
    final client = ForgeClient(
      baseUrl: Uri.parse('http://127.0.0.1:1'),
      store: MemoryStore(),
    );
    await expectLater(
      client.signIn('a@b.c', 'x'),
      throwsA(isA<ApiException>().having((e) => e.status, 'status', 0)),
    );
  });
}

/// Stockage qui échoue toujours (page web en `http` hors `localhost`).
class _BrokenStore implements KeyValueStore {
  @override
  Future<String?> read(String key) => Future.error(StateError('indisponible'));

  @override
  Future<void> write(String key, String? value) =>
      Future.error(StateError('indisponible'));
}
