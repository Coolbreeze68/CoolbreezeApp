/// Paramètres de liste de l'API : pagination, tri, recherche, filtres.
library;

/// Opérateur de filtre (`colonne[op]=valeur`).
enum FilterOp {
  eq,
  ne,
  lt,
  lte,
  gt,
  gte,

  /// Contient, sans tenir compte de la casse.
  like,

  /// Parmi une liste de valeurs.
  oneOf('in'),

  /// Valeur absente (`true`) ou présente (`false`).
  isNull('null');

  const FilterOp([this._key]);

  final String? _key;

  String get key => _key ?? name;
}

class Filter {
  const Filter(this.column, this.op, this.value);

  const Filter.equals(this.column, this.value) : op = FilterOp.eq;

  Filter.oneOf(this.column, Iterable<String> values)
    : op = FilterOp.oneOf,
      value = values.join(',');

  final String column;
  final FilterOp op;

  /// Valeur au format de l'API (date ISO, nombre avec un point…).
  final String value;

  String get key => op == FilterOp.eq ? column : '$column[${op.key}]';

  @override
  bool operator ==(Object other) =>
      other is Filter &&
      other.column == column &&
      other.op == op &&
      other.value == value;

  @override
  int get hashCode => Object.hash(column, op, value);
}

class Sort {
  const Sort(this.column, {this.descending = false});

  final String column;
  final bool descending;

  @override
  String toString() => descending ? '-$column' : column;
}

class ListQuery {
  const ListQuery({
    this.page = 1,
    this.perPage = 25,
    this.sort = const [],
    this.search,
    this.filters = const [],
  });

  /// Taille de page maximale acceptée par l'API.
  static const maxPerPage = 100;

  final int page;
  final int perPage;
  final List<Sort> sort;
  final String? search;
  final List<Filter> filters;

  ListQuery copyWith({
    int? page,
    int? perPage,
    List<Sort>? sort,
    String? search,
    List<Filter>? filters,
  }) => ListQuery(
    page: page ?? this.page,
    perPage: perPage ?? this.perPage,
    sort: sort ?? this.sort,
    search: search ?? this.search,
    filters: filters ?? this.filters,
  );

  /// Ajoute des filtres, remplaçant ceux de même clé.
  ListQuery where(Iterable<Filter> added) {
    final keys = {for (final f in added) f.key};
    return copyWith(
      page: 1,
      filters: [...filters.where((f) => !keys.contains(f.key)), ...added],
    );
  }

  /// Paramètres d'URL ; sans pagination pour les agrégats et l'export.
  Map<String, String> toParameters({bool paging = true}) => {
    if (paging) ...{'page': '$page', 'per_page': '$perPage'},
    if (sort.isNotEmpty) 'sort': sort.join(','),
    if (search?.trim().isNotEmpty == true) 'q': search!.trim(),
    for (final filter in filters) filter.key: filter.value,
  };

  /// Deux requêtes sont égales si elles produisent les mêmes paramètres.
  @override
  bool operator ==(Object other) {
    if (other is! ListQuery) return false;
    final mine = toParameters(), theirs = other.toParameters();
    return mine.length == theirs.length &&
        mine.entries.every((e) => theirs[e.key] == e.value);
  }

  @override
  int get hashCode => Object.hashAllUnordered(
    toParameters().entries.map((e) => Object.hash(e.key, e.value)),
  );
}

/// Page de résultats (le nom `Page` est pris par Flutter).
class Listing<T> {
  const Listing(
    this.items, {
    required this.page,
    required this.perPage,
    required this.total,
  });

  final List<T> items;
  final int page;
  final int perPage;
  final int total;

  int get pages => total == 0 ? 1 : (total + perPage - 1) ~/ perPage;
}
