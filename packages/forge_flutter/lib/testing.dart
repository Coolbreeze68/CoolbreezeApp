/// Aide aux tests des applications forge : une API en mémoire qui répond aux
/// routes déclarées par le test, et un client déjà connecté.
library;

import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'src/api/client.dart';
import 'src/api/store.dart';

/// Réponse explicite d'une route (statut autre que `200`).
class FakeResponse {
  const FakeResponse(this.status, [this.body]);

  final int status;
  final Object? body;
}

/// Traite une requête ; retourne le corps JSON, ou une [FakeResponse].
typedef FakeHandler = Object? Function(http.Request request, RegExpMatch match);

/// API simulée pour les tests : routes d'authentification intégrées
/// (mot de passe `secret`), autres routes déclarées par [on] ou [records].
class FakeApi {
  FakeApi({
    this.user = const {
      'id': 1,
      'email': 'admin@test',
      'roles': ['admin'],
    },
  }) {
    on('POST', '/api/auth/login', (request, _) {
      final body = jsonDecode(request.body) as Map;
      return body['password'] == 'secret'
          ? _session()
          : const FakeResponse(401, {
              'error': {
                'code': 'unauthorized',
                'message': 'authentification requise',
              },
            });
    });
    on('POST', '/api/auth/refresh', (_, _) => _session());
    on('POST', '/api/auth/logout', (_, _) => const FakeResponse(204));
    on('GET', '/api/auth/me', (_, _) => user);
  }

  static final baseUrl = Uri.parse('http://api.test');

  /// Compte renvoyé à la connexion.
  final Map<String, Object?> user;

  /// Requêtes reçues, dans l'ordre.
  final requests = <http.Request>[];

  final _routes = <(String, RegExp, FakeHandler)>[];

  /// Déclare une route ; `path` est une expression régulière sur le chemin
  /// entier (`/api/contact/(\d+)`). Les routes déclarées en dernier priment.
  void on(String method, String path, FakeHandler handler) =>
      _routes.insert(0, (method, RegExp('^$path\$'), handler));

  /// Liste paginée (filtre `id[in]` compris) et lecture par identifiant des
  /// enregistrements d'une table.
  void records(String table, List<Map<String, Object?>> records) {
    on('GET', '/api/$table', (request, _) {
      final params = request.url.queryParameters;
      final ids = params['id[in]']?.split(',').map(int.parse).toSet();
      final matching = [
        for (final record in records)
          if (ids == null || ids.contains(record['id'])) record,
      ];
      final page = int.parse(params['page'] ?? '1');
      final perPage = int.parse(params['per_page'] ?? '25');
      return {
        'data': matching.skip((page - 1) * perPage).take(perPage).toList(),
        'page': page,
        'per_page': perPage,
        'total': matching.length,
      };
    });
    on('GET', '/api/$table/(\\d+)', (request, match) {
      final id = int.parse(match[1]!);
      for (final record in records) {
        if (record['id'] == id) return record;
      }
      return const FakeResponse(404, {
        'error': {'code': 'not_found', 'message': 'enregistrement introuvable'},
      });
    });
  }

  http.Client get httpClient => MockClient((request) async {
    requests.add(request);
    for (final (method, path, handler) in _routes) {
      final match = path.firstMatch(request.url.path);
      if (method == request.method && match != null) {
        final result = handler(request, match);
        final (status, body) = switch (result) {
          FakeResponse(:final status, :final body) => (status, body),
          final body => (200, body),
        };
        return http.Response(
          body == null ? '' : jsonEncode(body),
          status,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      }
    }
    return http.Response(
      jsonEncode({
        'error': {
          'code': 'not_found',
          'message': '${request.method} ${request.url.path}',
        },
      }),
      404,
    );
  });

  /// Client de l'API simulée ; `signedIn` : une session est déjà enregistrée
  /// (reprise au démarrage de l'application).
  ForgeClient client({bool signedIn = true}) => ForgeClient(
    baseUrl: baseUrl,
    httpClient: httpClient,
    store: MemoryStore({if (signedIn) ForgeClient.refreshTokenKey: 'refresh'}),
  );

  Map<String, Object?> _session() => {
    'access_token': 'access',
    'refresh_token': 'refresh',
    'token_type': 'Bearer',
    'expires_in': 900,
    'user': user,
  };
}
