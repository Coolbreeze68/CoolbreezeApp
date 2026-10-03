import 'dart:typed_data';

import 'client.dart';
import 'file.dart';
import 'query.dart';

/// Opérations REST sur une table ; `decode` convertit un enregistrement JSON
/// en `T` (le JSON lui-même pour l'interface générique, un modèle typé de
/// `lib/generated/models.dart` pour le code personnalisé).
class TableClient<T> {
  const TableClient(this.client, this.table, this.decode);

  final ForgeClient client;
  final String table;
  final T Function(Json json) decode;

  String get _path => '/api/$table';

  Future<Listing<T>> list([ListQuery query = const ListQuery()]) async {
    final json = await client.get(_path, query: query.toParameters()) as Json;
    return Listing(
      [for (final item in json['data'] as List) decode(item as Json)],
      page: json['page'] as int,
      perPage: json['per_page'] as int,
      total: json['total'] as int,
    );
  }

  /// Tous les enregistrements correspondant à `query`, page par page, dans
  /// la limite de `max`.
  Future<List<T>> listAll(ListQuery query, {int max = 1000}) async {
    final items = <T>[];
    var page = 1;
    while (true) {
      final listing = await list(
        query.copyWith(page: page, perPage: ListQuery.maxPerPage),
      );
      items.addAll(listing.items);
      if (page >= listing.pages || items.length >= max) return items;
      page++;
    }
  }

  Future<T> read(int id) async =>
      decode(await client.get('$_path/$id') as Json);

  Future<T> create(Json values) async =>
      decode(await client.post(_path, values) as Json);

  /// Modification partielle : seules les colonnes de `values` changent.
  Future<T> update(int id, Json values) async =>
      decode(await client.patch('$_path/$id', values) as Json);

  Future<void> delete(int id) => client.delete('$_path/$id');

  /// Téléverse un fichier pour la colonne `file` ou `image` [column] ; son
  /// `id` s'écrit ensuite dans la colonne.
  Future<ForgeFile> upload(
    String column,
    String name,
    Uint8List bytes, {
    String contentType = 'application/octet-stream',
  }) async {
    final json = await client.postBytes(
      '/api/files',
      bytes,
      contentType: contentType,
      query: {'table': table, 'column': column, 'name': name},
    );
    return ForgeFile.fromJson(json)!;
  }

  Future<Aggregation> aggregate(
    List<String> fields, {
    String? groupBy,
    ListQuery query = const ListQuery(),
  }) async {
    final json =
        await client.get(
              '$_path/aggregate',
              query: {
                ...query.toParameters(paging: false),
                'fields': fields.join(','),
                'group_by': ?groupBy,
              },
            )
            as Json;
    return Aggregation.fromJson(json);
  }

  /// Export CSV des enregistrements correspondant à `query` (sans pagination).
  Future<Uint8List> export({
    ListQuery query = const ListQuery(),
    String delimiter = ',',
  }) => client.getBytes(
    '$_path/export',
    query: {...query.toParameters(paging: false), 'delimiter': delimiter},
  );

  /// Import CSV, tout ou rien ; une erreur détaille les lignes refusées
  /// ([ApiException.lines]).
  Future<ImportReport> import(Uint8List csv) async => ImportReport.fromJson(
    await client.postBytes('$_path/import', csv) as Json,
  );
}

class ImportReport {
  const ImportReport(this.created, this.updated);

  factory ImportReport.fromJson(Json json) =>
      ImportReport(json['created'] as int, json['updated'] as int);

  final int created;
  final int updated;
}

/// Somme, moyenne, minimum et maximum d'une colonne.
class Measure {
  const Measure({this.sum, this.avg, this.min, this.max});

  factory Measure.fromJson(Json json) => Measure(
    sum: _number(json['sum']),
    avg: _number(json['avg']),
    min: _number(json['min']),
    max: _number(json['max']),
  );

  final num? sum;
  final num? avg;
  final num? min;
  final num? max;
}

/// Les décimaux arrivent en texte, les entiers en nombre.
num? _number(Object? value) => switch (value) {
  final num n => n,
  final String s => num.tryParse(s),
  _ => null,
};

class AggregateGroup {
  const AggregateGroup(this.key, this.count, this.measures);

  factory AggregateGroup.fromJson(Json json, List<String> fields) =>
      AggregateGroup(json['key'], json['count'] as int, {
        for (final field in fields)
          if (json[field] is Map) field: Measure.fromJson(json[field] as Json),
      });

  /// Valeur de la colonne de regroupement (`null` : sans valeur).
  final Object? key;
  final int count;
  final Map<String, Measure> measures;
}

class Aggregation {
  const Aggregation({
    required this.fields,
    this.groupBy,
    this.groups = const [],
    required this.total,
  });

  factory Aggregation.fromJson(Json json) {
    final fields = [for (final f in json['fields'] as List) f as String];
    return Aggregation(
      fields: fields,
      groupBy: json['group_by'] as String?,
      groups: [
        for (final group in json['groups'] as List? ?? const [])
          AggregateGroup.fromJson(group as Json, fields),
      ],
      total: AggregateGroup.fromJson(json['total'] as Json, fields),
    );
  }

  final List<String> fields;
  final String? groupBy;
  final List<AggregateGroup> groups;

  /// Ensemble des enregistrements (sa clé est `null`).
  final AggregateGroup total;
}
