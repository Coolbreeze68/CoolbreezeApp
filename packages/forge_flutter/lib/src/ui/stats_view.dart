import 'package:flutter/material.dart';

import '../api/query.dart';
import '../api/table_client.dart';
import '../forge.dart';
import '../palette.dart';
import '../schema.dart';
import '../theme.dart';
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
      String details(ColumnSchema c, Measure? m) =>
          '${s.average} ${formatMeasure(forge, c, m?.avg)} · '
          '${s.min} ${formatMeasure(forge, c, m?.min)} · '
          '${s.max} ${formatMeasure(forge, c, m?.max)}';
      return ListView(
        padding: const EdgeInsets.all(16),
        children: [
          ResponsiveGrid(
            minWidth: 260,
            children: [
              StatTile(
                label: s.count,
                value: forge.format.number(aggregation.total.count),
                icon: Icons.tag,
              ),
              for (final column in fields)
                StatTile(
                  label: '${forge.format.columnLabel(column)} (${s.sum})',
                  value: formatMeasure(
                    forge,
                    column,
                    aggregation.total.measures[column.name]?.sum,
                  ),
                  details: details(
                    column,
                    aggregation.total.measures[column.name],
                  ),
                  icon: Icons.functions,
                  gradient: ForgeColors.tileGradients[1],
                ),
            ],
          ),
          if (groupBy != null && aggregation.groups.isNotEmpty) ...[
            const SizedBox(height: 16),
            ResponsiveGrid(
              minWidth: 420,
              children: [
                GroupBars(
                  title: '${s.count} · ${forge.format.columnLabel(groupBy)}',
                  table: table,
                  groupBy: groupBy,
                  groups: aggregation.groups,
                  value: (group) => group.count,
                  format: (n) => forge.format.number(n),
                ),
                for (final column in fields)
                  GroupBars(
                    title:
                        '${forge.format.columnLabel(column)} (${s.sum}) · '
                        '${forge.format.columnLabel(groupBy)}',
                    table: table,
                    groupBy: groupBy,
                    groups: aggregation.groups,
                    value: (group) => group.measures[column.name]?.sum ?? 0,
                    format: (n) => formatMeasure(forge, column, n),
                    tooltip: (group) =>
                        '${s.count} ${group.count} · '
                        '${details(column, group.measures[column.name])}',
                  ),
              ],
            ),
          ],
        ],
      );
    },
  );
}

/// Valeur d'une mesure, mise en forme selon la colonne (moyennes au centième).
String formatMeasure(Forge forge, ColumnSchema column, num? value) {
  if (value == null) return '–';
  return column.type == ColumnType.duration
      ? forge.format.duration(Duration(seconds: value.round()))
      : forge.format.number((value * 100).round() / 100);
}

/// Cartes disposées en colonnes, autant que la largeur le permet.
class ResponsiveGrid extends StatelessWidget {
  const ResponsiveGrid({
    super.key,
    required this.minWidth,
    required this.children,
  });

  final double minWidth;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      const gap = 16.0;
      final columns = ((constraints.maxWidth + gap) / (minWidth + gap))
          .floor()
          .clamp(1, 4);
      final width = (constraints.maxWidth - gap * (columns - 1)) / columns;
      return Wrap(
        spacing: gap,
        runSpacing: gap,
        children: [
          for (final child in children) SizedBox(width: width, child: child),
        ],
      );
    },
  );
}

/// Indicateur : icône en dégradé, valeur, détail.
class StatTile extends StatelessWidget {
  const StatTile({
    super.key,
    required this.label,
    required this.value,
    required this.icon,
    this.details,
    this.gradient,
    this.onTap,
  });

  final String label;
  final String value;
  final IconData icon;
  final String? details;
  final List<Color>? gradient;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return Card(
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                width: 46,
                height: 46,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(12),
                  gradient: LinearGradient(
                    begin: Alignment.topLeft,
                    end: Alignment.bottomRight,
                    colors: gradient ?? ForgeColors.tileGradients.first,
                  ),
                ),
                child: Icon(icon, color: Colors.white),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      label,
                      style: text.labelLarge?.copyWith(
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                    Text(
                      value,
                      style: text.headlineSmall?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                    if (details != null) Text(details!, style: text.bodySmall),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Barres horizontales, une par groupe. La couleur suit la valeur de
/// regroupement (énumération, dans l'ordre de déclaration) ; chaque barre
/// porte son libellé et sa valeur.
class GroupBars extends StatelessWidget {
  const GroupBars({
    super.key,
    required this.title,
    required this.table,
    required this.groupBy,
    required this.groups,
    required this.value,
    required this.format,
    this.tooltip,
  });

  final String title;
  final TableSchema table;
  final ColumnSchema groupBy;
  final List<AggregateGroup> groups;
  final num Function(AggregateGroup group) value;
  final String Function(num value) format;
  final String Function(AggregateGroup group)? tooltip;

  int _rank(AggregateGroup group) => switch (group.key) {
    null => 1 << 30,
    final key when groupBy.type == ColumnType.enumeration =>
      groupBy.values.indexOf('$key'),
    _ => 0,
  };

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final largest = groups
        .map((g) => value(g).abs())
        .fold<num>(0, (a, b) => a > b ? a : b);
    final ordered = [...groups]..sort((a, b) => _rank(a).compareTo(_rank(b)));
    Color color(AggregateGroup group) => switch (group.key) {
      null => Palette.neutral,
      final String key when groupBy.type == ColumnType.enumeration => enumColor(
        groupBy.values,
        key,
        theme.brightness,
      ),
      _ => theme.colorScheme.primary,
    };
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style: theme.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 12),
            for (final group in ordered)
              Tooltip(
                message: tooltip?.call(group) ?? format(value(group)),
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Row(
                    children: [
                      SizedBox(
                        width: 130,
                        child: _GroupLabel(table, groupBy, group.key),
                      ),
                      Expanded(
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: FractionallySizedBox(
                            widthFactor: largest == 0
                                ? 0.01
                                : (value(group).abs() / largest)
                                      .clamp(0.01, 1)
                                      .toDouble(),
                            child: Container(
                              height: 20,
                              decoration: BoxDecoration(
                                color: color(group),
                                borderRadius: const BorderRadius.horizontal(
                                  right: Radius.circular(4),
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      SizedBox(
                        width: 96,
                        child: Text(
                          format(value(group)),
                          textAlign: TextAlign.end,
                          style: const TextStyle(
                            fontWeight: FontWeight.w600,
                            fontFeatures: [FontFeature.tabularFigures()],
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
          ],
        ),
      ),
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
