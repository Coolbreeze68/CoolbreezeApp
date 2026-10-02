import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Stockage persistant de petites valeurs : jeton de rafraîchissement, langue.
abstract interface class KeyValueStore {
  Future<String?> read(String key);

  /// Écrit `value`, ou supprime la clé si `value` est `null`.
  Future<void> write(String key, String? value);
}

/// Stockage chiffré du système (Keychain, Keystore, WebCrypto sur le web).
class SecureStore implements KeyValueStore {
  const SecureStore([this._storage = const FlutterSecureStorage()]);

  final FlutterSecureStorage _storage;

  @override
  Future<String?> read(String key) => _storage.read(key: key);

  @override
  Future<void> write(String key, String? value) => value == null
      ? _storage.delete(key: key)
      : _storage.write(key: key, value: value);
}

/// Stockage en mémoire, perdu à la fermeture (tests, sessions éphémères).
class MemoryStore implements KeyValueStore {
  MemoryStore([Map<String, String>? values]) : values = values ?? {};

  final Map<String, String> values;

  @override
  Future<String?> read(String key) async => values[key];

  @override
  Future<void> write(String key, String? value) async {
    if (value == null) {
      values.remove(key);
    } else {
      values[key] = value;
    }
  }
}
