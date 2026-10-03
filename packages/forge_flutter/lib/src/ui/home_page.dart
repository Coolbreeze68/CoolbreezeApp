import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';

import '../api/query.dart';
import '../api/table_client.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import '../theme.dart';
import 'calendar_view.dart';
import 'shell.dart';
import 'stats_view.dart';

/// Accueil : bienvenue, indicateurs par table, répartitions et échéances.
class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  /// Nombre d'enregistrements, par table.
  Future<Map<String, int>>? _counts;
  Future<Map<String, Aggregation>>? _stats;
  Future<Map<String, List<CalendarEvent>>>? _upcoming;
  DataChanges? _changes;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
      _fetch();
    }
  }

  @override
  void dispose() {
    _changes?.removeListener(_reload);
    super.dispose();
  }

  List<TableSchema> get _readable {
    final forge = Forge.of(context);
    return [
      for (final t in forge.schema.tables)
        if (forge.can(t, Operation.read)) t,
    ];
  }

  void _reload() => setState(_fetch);

  void _fetch() {
    final client = Forge.of(context).client;
    final tables = _readable;
    final today = DateTime.now();
    final from = DateTime(today.year, today.month, today.day);
    {
      _counts = (() async => {
        for (final t in tables)
          t.name: (await client.table(t.name).list(const ListQuery(perPage: 1)))
              .total,
      })();
      _stats = (() async => {
        for (final t in tables)
          if (t.stats case StatsView(:final groupBy?, :final fields))
            t.name: await client
                .table(t.name)
                .aggregate(fields.take(1).toList(), groupBy: groupBy),
      })();
      _upcoming = (() async => {
        for (final t in tables)
          if (t.calendar != null)
            t.name: await loadEvents(
              client,
              t,
              const ListQuery(),
              from,
              DateTime(from.year, from.month, from.day + 30),
            ),
      })();
    }
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final theme = Theme.of(context);
    final tables = _readable;
    return ForgeScaffold(
      title: Text(s.dashboard),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Container(
            padding: const EdgeInsets.all(24),
            decoration: BoxDecoration(
              gradient: ForgeColors.vividGradient,
              borderRadius: BorderRadius.circular(16),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  forge.user == null
                      ? s.welcome
                      : '${s.welcome}, ${forge.user!.name}',
                  style: theme.textTheme.headlineSmall?.copyWith(
                    color: Colors.white,
                    fontWeight: FontWeight.w800,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  toBeginningOfSentenceCase(
                    DateFormat.yMMMMEEEEd(forge.locale).format(DateTime.now()),
                  ),
                  style: const TextStyle(color: Colors.white70),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          FutureBuilder(
            future: _counts,
            builder: (context, snapshot) => ResponsiveGrid(
              // Deux indicateurs par ligne sur un téléphone.
              minWidth: 160,
              children: [
                for (final (i, t) in tables.indexed)
                  StatTile(
                    label: forge.format.tableLabel(t),
                    value: switch (snapshot.data?[t.name]) {
                      final n? => forge.format.number(n),
                      null => '–',
                    },
                    icon:
                        forge.customization.tableIcons[t.name] ??
                        Icons.table_rows_outlined,
                    gradient: ForgeColors
                        .tileGradients[i % ForgeColors.tileGradients.length],
                    onTap: () => context.go(Paths.table(t.name)),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          FutureBuilder(
            future: Future.wait([_stats!, _upcoming!]),
            builder: (context, snapshot) {
              final data = snapshot.data;
              if (data == null) return const SizedBox.shrink();
              final stats = data[0] as Map<String, Aggregation>;
              final upcoming = data[1] as Map<String, List<CalendarEvent>>;
              return ResponsiveGrid(
                minWidth: 420,
                children: [
                  for (final MapEntry(key: name, value: aggregation)
                      in stats.entries)
                    _statsCard(forge, forge.schema.table(name)!, aggregation),
                  for (final MapEntry(key: name, value: events)
                      in upcoming.entries)
                    _UpcomingCard(
                      table: forge.schema.table(name)!,
                      events: events,
                    ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _statsCard(Forge forge, TableSchema table, Aggregation aggregation) {
    final field = table.column(aggregation.fields.first)!;
    return GroupBars(
      title:
          '${forge.format.tableLabel(table)} · '
          '${forge.format.columnLabel(field)} (${forge.strings.sum})',
      table: table,
      groupBy: table.column(aggregation.groupBy!)!,
      groups: aggregation.groups,
      value: (group) => group.measures[field.name]?.sum ?? 0,
      format: (n) => formatMeasure(forge, field, n),
    );
  }
}

/// Prochaines échéances d'une table à calendrier (30 jours).
class _UpcomingCard extends StatelessWidget {
  const _UpcomingCard({required this.table, required this.events});

  final TableSchema table;
  final List<CalendarEvent> events;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final theme = Theme.of(context);
    final start = table.column(table.calendar!.start)!;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              '${forge.format.tableLabel(table)} · '
              '${forge.format.columnLabel(start)}',
              style: theme.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 8),
            if (events.isEmpty)
              Text(forge.strings.noResults)
            else
              for (final event in events.take(6))
                ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  leading: TintedIcon(Icons.event, size: 34),
                  title: Text(
                    forge.format.title(table, event.record),
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                  subtitle: Text(
                    event.allDay
                        ? forge.format.date(event.start)
                        : forge.format.dateTime(event.start),
                  ),
                  onTap: () => context.push(
                    Paths.record(table.name, event.record['id'] as int),
                  ),
                ),
          ],
        ),
      ),
    );
  }
}
