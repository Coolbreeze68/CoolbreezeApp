import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'store.dart';
import 'table_client.dart';

typedef Json = Map<String, dynamic>;

/// Erreur renvoyée par l'API : `{ "error": { code, message, fields?, lines? } }`.
class ApiException implements Exception {
  ApiException(
    this.status,
    this.code,
    this.message, {
    this.fields = const {},
    this.lines = const [],
  });

  factory ApiException.fromResponse(http.Response response) {
    Object? body;
    try {
      body = jsonDecode(utf8.decode(response.bodyBytes));
    } on FormatException {
      body = null;
    }
    final error = body is Map && body['error'] is Map
        ? (body['error'] as Map).cast<String, dynamic>()
        : const <String, dynamic>{};
    return ApiException(
      response.statusCode,
      error['code'] as String? ?? 'http_${response.statusCode}',
      error['message'] as String? ?? response.reasonPhrase ?? '',
      fields: _fields(error['fields']),
      lines: [
        for (final line in error['lines'] as List? ?? const [])
          ImportLineError.fromJson((line as Map).cast<String, dynamic>()),
      ],
    );
  }

  /// Statut HTTP (`0` si le serveur est injoignable).
  final int status;
  final String code;
  final String message;

  /// Erreurs par champ d'une erreur de validation.
  final Map<String, List<String>> fields;

  /// Erreurs par ligne d'un import refusé.
  final List<ImportLineError> lines;

  bool get isUnauthorized => status == 401;

  @override
  String toString() => 'ApiException($status, $code): $message';
}

Map<String, List<String>> _fields(Object? raw) => {
  if (raw is Map)
    for (final MapEntry(:key, :value) in raw.entries)
      key as String: [for (final message in value as List) message as String],
};

/// Ligne refusée d'un import CSV (la ligne d'en-tête est la ligne 1).
class ImportLineError {
  const ImportLineError(this.line, this.message, this.fields);

  factory ImportLineError.fromJson(Json json) => ImportLineError(
    json['line'] as int,
    json['message'] as String? ?? '',
    _fields(json['fields']),
  );

  final int line;
  final String message;
  final Map<String, List<String>> fields;
}

/// Compte connecté.
class ForgeUser {
  const ForgeUser({
    required this.id,
    required this.email,
    this.displayName,
    this.active = true,
    this.roles = const [],
  });

  factory ForgeUser.fromJson(Json json) => ForgeUser(
    id: json['id'] as int,
    email: json['email'] as String,
    displayName: json['display_name'] as String?,
    active: json['active'] as bool? ?? true,
    roles: [for (final role in json['roles'] as List? ?? const []) '$role'],
  );

  final int id;
  final String email;
  final String? displayName;
  final bool active;
  final List<String> roles;

  String get name =>
      displayName?.trim().isNotEmpty == true ? displayName! : email;
}

/// Client de l'API d'une application forge : session (jeton d'accès renouvelé
/// automatiquement) et requêtes JSON. Notifie ses auditeurs à l'ouverture et à
/// la fermeture de la session.
class ForgeClient extends ChangeNotifier {
  ForgeClient({
    required this.baseUrl,
    http.Client? httpClient,
    KeyValueStore? store,
  }) : _http = httpClient ?? http.Client(),
       store = store ?? const SecureStore();

  /// Clé du jeton de rafraîchissement dans [store].
  static const refreshTokenKey = 'forge.refresh_token';

  final Uri baseUrl;
  final KeyValueStore store;
  final http.Client _http;

  String? _accessToken;
  String? _refreshToken;
  ForgeUser? _user;
  Future<bool>? _refreshing;

  ForgeUser? get user => _user;

  bool get signedIn => _user != null;

  /// Opérations sur une table, enregistrements en JSON.
  TableClient<Json> table(String name) =>
      TableClient(this, name, (json) => json);

  Future<ForgeUser> signIn(String email, String password) async {
    final session = await _send(
      'POST',
      '/api/auth/login',
      body: {'email': email, 'password': password},
      authenticated: false,
    );
    return _open(session as Json);
  }

  /// Reprend la session enregistrée, s'il y en a une encore valide.
  Future<bool> restore() async {
    try {
      _refreshToken = await store.read(refreshTokenKey);
    } catch (error) {
      debugPrint('forge : session enregistrée illisible ($error)');
      return false;
    }
    return _refreshToken != null && await _refresh();
  }

  Future<void> signOut() async {
    final token = _refreshToken;
    if (token != null) {
      try {
        await _send(
          'POST',
          '/api/auth/logout',
          body: {'refresh_token': token},
          authenticated: false,
        );
      } on ApiException {
        // La session locale est fermée même si le serveur ne répond pas.
      }
    }
    await _close();
  }

  /// Recharge le compte connecté (rôles, nom).
  Future<ForgeUser> reloadUser() async {
    _user = ForgeUser.fromJson(await get('/api/auth/me') as Json);
    notifyListeners();
    return _user!;
  }

  Future<Object?> get(String path, {Map<String, String>? query}) =>
      _send('GET', path, query: query);

  Future<Object?> post(String path, Object? body) =>
      _send('POST', path, body: body);

  Future<Object?> put(String path, Object? body) =>
      _send('PUT', path, body: body);

  Future<Object?> patch(String path, Object? body) =>
      _send('PATCH', path, body: body);

  Future<void> delete(String path) => _send('DELETE', path);

  /// Corps brut d'une réponse (export CSV).
  Future<Uint8List> getBytes(String path, {Map<String, String>? query}) async {
    final response = await _authenticated(
      () => http.Request('GET', _uri(path, query)),
    );
    return response.bodyBytes;
  }

  /// Envoie un corps brut (import CSV) et décode la réponse JSON.
  Future<Object?> postBytes(
    String path,
    Uint8List bytes, {
    String contentType = 'text/csv',
  }) async {
    final response = await _authenticated(
      () => http.Request('POST', _uri(path, null))
        ..headers['content-type'] = contentType
        ..bodyBytes = bytes,
    );
    return _decode(response);
  }

  Uri _uri(String path, Map<String, String>? query) => baseUrl.replace(
    path: '${baseUrl.path.replaceAll(RegExp(r'/$'), '')}$path',
    queryParameters: query == null || query.isEmpty ? null : query,
  );

  Future<Object?> _send(
    String method,
    String path, {
    Map<String, String>? query,
    Object? body,
    bool authenticated = true,
  }) async {
    http.Request build() {
      final request = http.Request(method, _uri(path, query))
        ..headers['accept'] = 'application/json';
      if (body != null) {
        request
          ..headers['content-type'] = 'application/json'
          ..body = jsonEncode(body);
      }
      return request;
    }

    final response = authenticated
        ? await _authenticated(build)
        : await _execute(build());
    return _decode(response);
  }

  /// Exécute la requête avec le jeton d'accès ; sur `401`, renouvelle la
  /// session une fois et rejoue la requête.
  Future<http.Response> _authenticated(http.Request Function() build) async {
    Future<http.Response> attempt() {
      final request = build();
      if (_accessToken != null) {
        request.headers['authorization'] = 'Bearer $_accessToken';
      }
      return _execute(request);
    }

    try {
      return await attempt();
    } on ApiException catch (error) {
      if (!error.isUnauthorized || _refreshToken == null) rethrow;
      if (!await _refresh()) rethrow;
      return attempt();
    }
  }

  Future<http.Response> _execute(http.BaseRequest request) async {
    final http.Response response;
    try {
      response = await http.Response.fromStream(await _http.send(request));
    } on http.ClientException catch (error) {
      throw ApiException(0, 'network', error.message);
    }
    if (response.statusCode >= 400) throw ApiException.fromResponse(response);
    return response;
  }

  Object? _decode(http.Response response) => response.bodyBytes.isEmpty
      ? null
      : jsonDecode(utf8.decode(response.bodyBytes));

  /// Renouvelle la session ; les appels simultanés partagent le même renouvellement.
  Future<bool> _refresh() => _refreshing ??= () async {
    try {
      final session = await _send(
        'POST',
        '/api/auth/refresh',
        body: {'refresh_token': _refreshToken},
        authenticated: false,
      );
      await _open(session as Json);
      return true;
    } on ApiException catch (error) {
      // Serveur injoignable : la session reste valable pour un nouvel essai.
      if (error.status == 0) return false;
      await _close();
      return false;
    } finally {
      _refreshing = null;
    }
  }();

  Future<ForgeUser> _open(Json session) async {
    _accessToken = session['access_token'] as String;
    _refreshToken = session['refresh_token'] as String;
    _user = ForgeUser.fromJson(session['user'] as Json);
    notifyListeners();
    await _remember(_refreshToken);
    return _user!;
  }

  /// Enregistre le jeton de rafraîchissement. Un échec (stockage chiffré
  /// indisponible, par exemple sur une page web en `http` hors `localhost`)
  /// n'empêche pas la session : elle ne sera simplement pas reprise.
  Future<void> _remember(String? token) async {
    try {
      await store.write(refreshTokenKey, token);
    } catch (error) {
      debugPrint('forge : session non enregistrée ($error)');
    }
  }

  Future<void> _close() async {
    final wasSignedIn = signedIn;
    _accessToken = _refreshToken = _user = null;
    if (wasSignedIn) notifyListeners();
    await _remember(null);
  }

  @override
  void dispose() {
    _http.close();
    super.dispose();
  }
}
