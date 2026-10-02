import 'package:flutter/material.dart';

import '../api/query.dart';
import '../api/table_client.dart';
import '../forge.dart';
import '../schema.dart';
import 'widgets.dart';

/// Statistiques de la vue `stats` : totaux et répartition par groupe, sur les
/// enregistrements correspondant à la recherche et aux filtres.
class StatsPanel extends StatefulWidget {
  const StatsPanel({super.key, required this.table, required this.query});

  final TableSchema table;
  final ListQuery query;

  @override
  State<StatsPanel> createState() => _StatsPanelState();
}

class _StatsPanelState extends State<StatsPanel> {
  Future<Aggregation>? _aggregation;
  DataChanges? _changes;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
    }
    _aggregation ??= _load();
  }

  @override
  void didUpdateWidget(StatsPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.query != widget.query) _aggregation = _load();
  }

  @override
  void dispose() {
    _changes?.removeListener(_reload);
    super.dispose();
  }

  Future<Aggregation> _load() {
    final view = widget.table.stats!;
    return Forge.of(context).client
        .table(widget.table.name)
        .aggregate(view.fields, groupBy: view.groupBy, query: widget.query);
  }

  void _reload() => setState(() {
    _aggregation = _load();
  });

  @override
  Widget build(BuildContext context) => FutureBuilder(
    future: _aggregation,
    builder: (context, snapshot) {
      if (snapshot.hasError) {
        return ErrorView(snapshot.error!, onRetry: _reload);
      }
      final aggregation = snapshot.data;
      if (aggregation == null) {
        return const Center(child: CircularProgressIndicator());
      }
      final forge = Forge.of(context);
      final s = forge.strings;
      final table = widget.table;
      final groupBy = switch (aggregation.groupBy) {
        final name? => table.column(name),
        null => null,
      };
      final fields = [
        for (final name in aggregation.fields) ?table.column(name),
      ];
      return ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Wrap(
            spacing: 12,
            runSpacing: 12,
            children: [
              _Tile(
                label: s.count,
                value: forge.format.number(aggregation.total.count),
              ),
              for (final column in fields)
                _MeasureTile(
                  column: column,
                  measure: aggregation.total.measures[column.name],
                ),
            ],
          ),
          if (groupBy != null && aggregation.groups.isNotEmpty) ...[
            const SizedBox(height: 24),
            _Bars(
              title: '${s.count} · ${forge.format.columnLabel(groupBy)}',
              table: table,
              groupBy: groupBy,
              groups: aggregation.groups,
              value: (group) => group.count,
              format: (n) => forge.format.number(n),
            ),
            for (final column in fields) ...[
              const SizedBox(height: 24),
              _Bars(
                title:
                    '${forge.format.columnLabel(column)} (${s.sum}) · '
                    '${forge.format.columnLabel(groupBy)}',
                table: table,
                groupBy: groupBy,
                groups: aggregation.groups,
                value: (group) => group.measures[column.name]?.sum ?? 0,
                format: (n) => _formatMeasure(forge, column, n),
              ),
            ],
          ],
        ],
      );
    },
  );
}

/// Moyenne arrondie au centième.
num? _round(num? value) => value == null ? null : (value * 100).round() / 100;

String _formatMeasure(Forge forge, ColumnSchema column, num? value) {
  if (value == null) return '–';
  return column.type == ColumnType.duration
      ? forge.format.duration(Duration(seconds: value.round()))
      : forge.format.number(value);
}

class _Tile extends StatelessWidget {
  const _Tile({required this.label, required this.value, this.details});

  final String label;
  final String value;
  final String? details;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return SizedBox(
      width: 240,
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(label, style: text.labelLarge),
              const SizedBox(height: 4),
              Text(value, style: text.headlineSmall),
              if (details != null) ...[
                const SizedBox(height: 4),
                Text(details!, style: text.bodySmall),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _MeasureTile extends StatelessWidget {
  const _MeasureTile({required this.column, required this.measure});

  final ColumnSchema column;
  final Measure? measure;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    String f(num? n) => _formatMeasure(forge, column, n);
    return _Tile(
      label: '${forge.format.columnLabel(column)} (${s.sum})',
      value: f(measure?.sum),
      details:
          '${s.average} ${f(_round(measure?.avg))} · ${s.min} ${f(measure?.min)} · '
          '${s.max} ${f(measure?.max)}',
    );
  }
}

/// Barres horizontales, une par groupe.
class _Bars extends StatelessWidget {
  const _Bars({
    required this.title,
    required this.table,
    required this.groupBy,
    required this.groups,
    required this.value,
    required this.format,
  });

  final String title;
  final TableSchema table;
  final ColumnSchema groupBy;
  final List<AggregateGroup> groups;
  final num Function(AggregateGroup group) value;
  final String Function(num value) format;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final largest = groups
        .map((g) => value(g).abs())
        .fold<num>(0, (a, b) => a > b ? a : b);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 8),
        for (final group in groups)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 3),
            child: Row(
              children: [
                SizedBox(
                  width: 160,
                  child: _GroupLabel(table, groupBy, group.key),
                ),
                Expanded(
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: FractionallySizedBox(
                      widthFactor: largest == 0
                          ? 0
                          : (value(group).abs() / largest)
                                .clamp(0, 1)
                                .toDouble(),
                      child: Container(
                        height: 18,
                        color: value(group) < 0 ? colors.error : colors.primary,
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                SizedBox(
                  width: 110,
                  child: Text(format(value(group)), textAlign: TextAlign.end),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// Libellé d'une valeur de regroupement.
class _GroupLabel extends StatelessWidget {
  const _GroupLabel(this.table, this.column, this.value);

  final TableSchema table;
  final ColumnSchema column;
  final Object? value;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final key = value;
    if (key == null) return Text(forge.strings.none);
    if (column.type == ColumnType.reference && key is int) {
      return RecordTitle(column.target!, key);
    }
    return Text(
      forge.format.format(table, column, key),
      overflow: TextOverflow.ellipsis,
    );
  }
}
