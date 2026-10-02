import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/client.dart';
import '../api/query.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import 'widgets.dart';

/// Liste paginée des enregistrements de `table` correspondant à `query`
/// (sa page est ignorée) : tableau sur grand écran, tuiles sinon. Se recharge
/// après chaque écriture.
class RecordList extends StatefulWidget {
  const RecordList({
    super.key,
    required this.table,
    this.query = const ListQuery(),
    this.compact = false,
    this.onSort,
  });

  final TableSchema table;
  final ListQuery query;

  /// Tuiles même sur grand écran (listes liées d'une page de détail).
  final bool compact;

  /// Tri demandé en cliquant un en-tête de colonne (tableau seulement).
  final ValueChanged<List<Sort>>? onSort;

  @override
  State<RecordList> createState() => _RecordListState();
}

class _RecordListState extends State<RecordList> {
  int _page = 1;
  Future<Listing<Json>>? _listing;
  DataChanges? _changes;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
    }
    _listing ??= _load();
  }

  @override
  void didUpdateWidget(RecordList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.query != widget.query || oldWidget.table != widget.table) {
      _page = 1;
      _listing = _load();
    }
  }

  @override
  void dispose() {
    _changes?.removeListener(_reload);
    super.dispose();
  }

  void _reload() => setState(() {
    _listing = _load();
  });

  Future<Listing<Json>> _load() => Forge.of(
    context,
  ).client.table(widget.table.name).list(widget.query.copyWith(page: _page));

  @override
  Widget build(BuildContext context) => FutureBuilder(
    future: _listing,
    builder: (context, snapshot) {
      if (snapshot.hasError) {
        return ErrorView(snapshot.error!, onRetry: _reload);
      }
      final listing = snapshot.data;
      if (listing == null) {
        return const Padding(
          padding: EdgeInsets.all(24),
          child: Center(child: CircularProgressIndicator()),
        );
      }
      if (listing.items.isEmpty) {
        return Padding(
          padding: const EdgeInsets.all(24),
          child: Center(child: Text(Forge.of(context).strings.noResults)),
        );
      }
      final first = (listing.page - 1) * listing.perPage + 1;
      final pager = Pager(
        page: listing.page,
        pages: listing.pages,
        first: first,
        last: first + listing.items.length - 1,
        total: listing.total,
        onPage: (page) => setState(() {
          _page = page;
          _listing = _load();
        }),
      );
      final wide = !widget.compact && isWide(context);
      final records = wide
          ? _RecordTable(
              table: widget.table,
              records: listing.items,
              sort: widget.query.sort,
              onSort: widget.onSort,
            )
          : _RecordTiles(
              table: widget.table,
              records: listing.items,
              scrollable: !widget.compact,
            );
      if (widget.compact) {
        return Column(children: [records, if (listing.pages > 1) pager]);
      }
      return Column(
        children: [
          Expanded(child: records),
          // Laisse la place du bouton de création.
          Padding(padding: const EdgeInsets.only(right: 88), child: pager),
        ],
      );
    },
  );
}

void _open(BuildContext context, TableSchema table, Json record) =>
    context.push(Paths.record(table.name, record['id'] as int));

class _RecordTable extends StatelessWidget {
  const _RecordTable({
    required this.table,
    required this.records,
    required this.sort,
    required this.onSort,
  });

  final TableSchema table;
  final List<Json> records;
  final List<Sort> sort;
  final ValueChanged<List<Sort>>? onSort;

  @override
  Widget build(BuildContext context) {
    final format = Forge.of(context).format;
    final columns = table.visibleColumns.toList();
    final sorted = sort.firstOrNull;
    final sortIndex = sorted == null
        ? null
        : columns.indexWhere((c) => c.name == sorted.column);
    return SingleChildScrollView(
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: DataTable(
          showCheckboxColumn: false,
          sortColumnIndex: sortIndex == null || sortIndex < 0
              ? null
              : sortIndex,
          sortAscending: !(sorted?.descending ?? false),
          columns: [
            for (final column in columns)
              DataColumn(
                label: Text(format.columnLabel(column)),
                numeric: column.type.isNumeric,
                onSort: column.stored && onSort != null
                    ? (_, ascending) =>
                          onSort!([Sort(column.name, descending: !ascending)])
                    : null,
              ),
          ],
          rows: [
            for (final record in records)
              DataRow(
                onSelectChanged: (_) => _open(context, table, record),
                cells: [
                  for (final column in columns)
                    DataCell(
                      ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 280),
                        child: ValueView(
                          table: table,
                          column: column,
                          record: record,
                          links: false,
                          maxLines: 1,
                        ),
                      ),
                    ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}

class _RecordTiles extends StatelessWidget {
  const _RecordTiles({
    required this.table,
    required this.records,
    required this.scrollable,
  });

  final TableSchema table;
  final List<Json> records;

  /// Faux dans une page qui défile déjà.
  final bool scrollable;

  @override
  Widget build(BuildContext context) {
    final format = Forge.of(context).format;
    // Sous-titre : premières colonnes renseignées, hors intitulé et références.
    final details = table.visibleColumns
        .where(
          (c) =>
              !c.titleField &&
              c.type != ColumnType.reference &&
              c.type != ColumnType.referenceList &&
              c.type != ColumnType.text,
        )
        .toList();
    return ListView.separated(
      shrinkWrap: !scrollable,
      physics: scrollable ? null : const NeverScrollableScrollPhysics(),
      itemCount: records.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, index) {
        final record = records[index];
        final subtitle = details
            .map((c) => (c, format.format(table, c, record[c.name])))
            .where((entry) => entry.$2.isNotEmpty)
            .take(2)
            .map((entry) => '${format.columnLabel(entry.$1)} : ${entry.$2}')
            .join(' · ');
        return ListTile(
          title: Text(format.title(table, record)),
          subtitle: subtitle.isEmpty ? null : Text(subtitle, maxLines: 1),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => _open(context, table, record),
        );
      },
    );
  }
}
