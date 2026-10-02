import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/query.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import 'calendar_view.dart';
import 'csv.dart';
import 'filters.dart';
import 'record_list.dart';
import 'shell.dart';
import 'stats_view.dart';
import 'widgets.dart';

enum TableView { list, calendar, stats }

/// Page d'une table : liste, calendrier et statistiques, qui partagent la
/// recherche et les filtres.
class TablePage extends StatefulWidget {
  const TablePage({
    super.key,
    required this.table,
    this.initialFilters = const {},
  });

  final TableSchema table;

  /// Filtres d'égalité lus dans l'URL (`?entreprise=3`).
  final Map<String, String> initialFilters;

  @override
  State<TablePage> createState() => _TablePageState();
}

class _TablePageState extends State<TablePage> {
  var _view = TableView.list;
  var _search = '';
  late final Map<String, ColumnFilter> _filters = {
    for (final column in filterableColumns(widget.table))
      if (widget.initialFilters[column.name] case final value?)
        column.name: ?ColumnFilter.fromParameter(column, value),
  };
  List<Sort> _sort = const [];
  Timer? _debounce;

  TableSchema get table => widget.table;

  ListQuery get _query => ListQuery(
    sort: _sort,
    search: _search,
    filters: [for (final filter in _filters.values) ...filter.filters],
  );

  @override
  void dispose() {
    _debounce?.cancel();
    super.dispose();
  }

  void _onSearch(String text) {
    _debounce?.cancel();
    _debounce = Timer(
      const Duration(milliseconds: 300),
      () => setState(() => _search = text),
    );
  }

  Future<void> _editFilter(ColumnSchema column) async {
    final result = await editFilter(
      context,
      table,
      column,
      _filters[column.name],
    );
    if (result.cancelled) return;
    setState(() {
      if (result.filter case final filter?) {
        _filters[column.name] = filter;
      } else {
        _filters.remove(column.name);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final views = [
      TableView.list,
      if (table.calendar != null) TableView.calendar,
      if (table.stats != null) TableView.stats,
    ];
    final canCreate = forge.can(table, Operation.create);
    final canImport = canCreate || forge.can(table, Operation.update);
    final wide = isWide(context);
    final switcher = SegmentedButton<TableView>(
      showSelectedIcon: false,
      segments: [
        for (final view in views)
          ButtonSegment(
            value: view,
            tooltip: switch (view) {
              TableView.list => s.listView,
              TableView.calendar => s.calendarView,
              TableView.stats => s.statsView,
            },
            icon: Icon(switch (view) {
              TableView.list => Icons.view_list,
              TableView.calendar => Icons.calendar_month,
              TableView.stats => Icons.bar_chart,
            }),
          ),
      ],
      selected: {_view},
      onSelectionChanged: (v) => setState(() => _view = v.first),
    );
    return ForgeScaffold(
      title: Text(forge.format.tableLabel(table)),
      actions: [
        PopupMenuButton<VoidCallback>(
          tooltip: s.more,
          onSelected: (action) => action(),
          itemBuilder: (context) => [
            PopupMenuItem(
              value: () => exportCsv(context, table, _query),
              child: ListTile(
                leading: const Icon(Icons.download),
                title: Text(s.exportCsv),
              ),
            ),
            if (canImport)
              PopupMenuItem(
                value: () => importCsv(context, table),
                child: ListTile(
                  leading: const Icon(Icons.upload),
                  title: Text(s.importCsv),
                ),
              ),
          ],
        ),
      ],
      floatingActionButton: canCreate && _view != TableView.stats
          ? FloatingActionButton(
              tooltip: s.create,
              onPressed: () => context.push(Paths.create(table.name)),
              child: const Icon(Icons.add),
            )
          : null,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: Row(
              children: [
                if (table.searchable)
                  Expanded(
                    child: TextField(
                      decoration: InputDecoration(
                        prefixIcon: const Icon(Icons.search),
                        hintText: s.search,
                        isDense: true,
                      ),
                      onChanged: _onSearch,
                    ),
                  )
                else
                  const Spacer(),
                PopupMenuButton<ColumnSchema>(
                  tooltip: s.filter,
                  icon: const Icon(Icons.filter_list),
                  onSelected: _editFilter,
                  itemBuilder: (context) => [
                    for (final column in filterableColumns(table))
                      PopupMenuItem(
                        value: column,
                        child: Text(forge.format.columnLabel(column)),
                      ),
                  ],
                ),
                if (views.length > 1 && wide) switcher,
              ],
            ),
          ),
          // Sur petit écran, le choix de la vue a sa propre ligne.
          if (views.length > 1 && !wide)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
              child: switcher,
            ),
          if (_filters.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
              child: Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final filter in _filters.values)
                    FilterChipView(
                      table: table,
                      filter: filter,
                      onEdit: () => _editFilter(filter.column),
                      onDeleted: () =>
                          setState(() => _filters.remove(filter.column.name)),
                    ),
                ],
              ),
            ),
          const SizedBox(height: 8),
          Expanded(
            child: switch (_view) {
              TableView.list => RecordList(
                table: table,
                query: _query,
                onSort: (sort) => setState(() => _sort = sort),
              ),
              TableView.calendar => CalendarPanel(table: table, query: _query),
              TableView.stats => StatsPanel(table: table, query: _query),
            },
          ),
        ],
      ),
    );
  }
}
