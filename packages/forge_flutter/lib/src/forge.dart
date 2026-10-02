import 'dart:async';

import 'package:flutter/widgets.dart';

import 'api/client.dart';
import 'api/query.dart';
import 'customization.dart';
import 'l10n/strings.dart';
import 'schema.dart';
import 'values.dart';

/// Contexte de l'application, accessible par `Forge.of(context)` dans toutes
/// les pages, y compris les pages et widgets personnalisés.
class Forge extends InheritedWidget {
  const Forge({
    super.key,
    required this.schema,
    required this.client,
    required this.customization,
    required this.format,
    required this.titles,
    required this.changes,
    required this.user,
    required this.locale,
    required this.setLocale,
    required super.child,
  });

  final AppSchema schema;
  final ForgeClient client;
  final ForgeCustomization customization;
  final ValueFormat format;
  final TitleCache titles;
  final DataChanges changes;
  final ForgeUser? user;
  final String locale;
  final ValueChanged<String> setLocale;

  static Forge of(BuildContext context) {
    final forge = context.dependOnInheritedWidgetOfExactType<Forge>();
    assert(forge != null, 'Forge.of appelé hors de ForgeApp');
    return forge!;
  }

  ForgeStrings get strings => format.strings;

  List<String> get roles => user?.roles ?? const [];

  bool get isAdmin => roles.contains(adminRole);

  bool can(TableSchema table, Operation operation) =>
      table.allows(roles, operation);

  @override
  bool updateShouldNotify(Forge oldWidget) =>
      format != oldWidget.format ||
      user != oldWidget.user ||
      customization != oldWidget.customization ||
      schema != oldWidget.schema;
}

/// Signale les écritures : les pages ouvertes se rechargent, une écriture
/// pouvant changer des valeurs calculées d'autres tables.
class DataChanges extends ChangeNotifier {
  void notify() => notifyListeners();
}

/// Intitulés des enregistrements visés par des références, chargés par lots :
/// les demandes faites pendant une même construction d'écran partent en une
/// requête par table.
class TitleCache {
  TitleCache(this._client, this._schema, this._format);

  final ForgeClient _client;
  final AppSchema _schema;
  final ValueFormat _format;
  final _titles = <String, Map<int, Future<String>>>{};
  final _pending = <String, Map<int, Completer<String>>>{};

  /// Intitulé d'un enregistrement, ou `#id` s'il est illisible.
  Future<String> title(String table, int id) {
    final known = _titles.putIfAbsent(table, () => {});
    return known[id] ??= () {
      final pending = _pending.putIfAbsent(table, () => {});
      if (pending.isEmpty) scheduleMicrotask(() => _flush(table));
      return (pending[id] = Completer<String>()).future;
    }();
  }

  /// Oublie les intitulés (après une écriture).
  void clear() => _titles.clear();

  Future<void> _flush(String table) async {
    final pending = _pending.remove(table) ?? {};
    final schema = _schema.table(table);
    final titles = <int, String>{};
    if (schema != null) {
      final ids = pending.keys.toList();
      try {
        for (var i = 0; i < ids.length; i += ListQuery.maxPerPage) {
          final chunk = ids.skip(i).take(ListQuery.maxPerPage);
          final listing = await _client
              .table(table)
              .list(
                ListQuery(
                  perPage: ListQuery.maxPerPage,
                  filters: [Filter.oneOf('id', chunk.map((id) => '$id'))],
                ),
              );
          for (final record in listing.items) {
            titles[record['id'] as int] = _format.title(schema, record);
          }
        }
      } on ApiException {
        // Table illisible pour cet utilisateur : intitulés par défaut.
      }
    }
    for (final MapEntry(key: id, value: completer) in pending.entries) {
      completer.complete(titles[id] ?? '#$id');
    }
  }
}
