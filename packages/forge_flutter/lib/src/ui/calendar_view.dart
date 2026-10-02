import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';

import '../api/client.dart';
import '../api/query.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import '../values.dart';
import 'widgets.dart';

/// Enregistrement placé dans le calendrier.
class CalendarEvent {
  const CalendarEvent(
    this.record,
    this.start,
    this.end, {
    required this.allDay,
  });

  final Json record;
  final DateTime start;

  /// Fin, si la vue en a une (colonne de fin ou durée).
  final DateTime? end;

  /// Colonne de type date : pas d'heure.
  final bool allDay;

  DateTime get _lastDay => _day(end ?? start);

  bool occursOn(DateTime day) =>
      !day.isBefore(_day(start)) && !day.isAfter(_lastDay);
}

DateTime _day(DateTime t) => DateTime(t.year, t.month, t.day);

/// Événements de `table` dont une partie tombe dans [from, to[.
Future<List<CalendarEvent>> loadEvents(
  ForgeClient client,
  TableSchema table,
  ListQuery query,
  DateTime from,
  DateTime to,
) async {
  final view = table.calendar!;
  final start = table.column(view.start)!;
  final allDay = start.type == ColumnType.date;
  String bound(DateTime day) =>
      allDay ? dateToJson(day)! : dateTimeToJson(day)!;
  final records = client.table(table.name);
  final base = query.copyWith(sort: [Sort(view.start)]);
  final found = <int, Json>{
    for (final record in await records.listAll(
      base.where([
        Filter(view.start, FilterOp.gte, bound(from)),
        Filter(view.start, FilterOp.lt, bound(to)),
      ]),
    ))
      record['id'] as int: record,
  };
  // Commencés avant la période et pas encore finis.
  if (view.end case final end?) {
    for (final record in await records.listAll(
      base.where([
        Filter(view.start, FilterOp.lt, bound(from)),
        Filter(end, FilterOp.gte, bound(from)),
      ]),
    )) {
      found[record['id'] as int] = record;
    }
  }
  DateTime? read(Object? json) =>
      allDay ? jsonToDate(json) : jsonToDateTime(json);
  return [
    for (final record in found.values)
      if (read(record[view.start]) case final begin?)
        CalendarEvent(record, begin, switch ((view.end, view.duration)) {
          (final end?, _) => read(record[end]),
          (_, final duration?) => switch (jsonToDuration(record[duration])) {
            final d? => begin.add(d),
            null => null,
          },
          _ => null,
        }, allDay: allDay),
  ]..sort((a, b) => a.start.compareTo(b.start));
}

/// Vue mensuelle et agenda du jour choisi.
class CalendarPanel extends StatefulWidget {
  const CalendarPanel({super.key, required this.table, required this.query});

  final TableSchema table;
  final ListQuery query;

  @override
  State<CalendarPanel> createState() => _CalendarPanelState();
}

class _CalendarPanelState extends State<CalendarPanel> {
  late DateTime _selected = _day(DateTime.now());
  late DateTime _month = DateTime(_selected.year, _selected.month);
  Future<List<CalendarEvent>>? _events;
  DataChanges? _changes;

  /// Premier jour affiché : le lundi de la semaine du 1er du mois.
  DateTime get _gridStart =>
      DateTime(_month.year, _month.month, 1 - (_month.weekday - 1));

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
    }
    _events ??= _load();
  }

  @override
  void didUpdateWidget(CalendarPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.query != widget.query) _events = _load();
  }

  @override
  void dispose() {
    _changes?.removeListener(_reload);
    super.dispose();
  }

  Future<List<CalendarEvent>> _load() {
    final start = _gridStart;
    return loadEvents(
      Forge.of(context).client,
      widget.table,
      widget.query,
      start,
      DateTime(start.year, start.month, start.day + 42),
    );
  }

  void _reload() => setState(() {
    _events = _load();
  });

  void _showMonth(DateTime month, {DateTime? select}) => setState(() {
    _month = DateTime(month.year, month.month);
    _selected = select ?? _month;
    _events = _load();
  });

  @override
  Widget build(BuildContext context) => FutureBuilder(
    future: _events,
    builder: (context, snapshot) {
      if (snapshot.hasError) {
        return ErrorView(snapshot.error!, onRetry: _reload);
      }
      final events = snapshot.data ?? const <CalendarEvent>[];
      final grid = Column(
        children: [
          _header(context, loading: !snapshot.hasData),
          Expanded(child: _grid(context, events)),
        ],
      );
      final agenda = _Agenda(
        table: widget.table,
        day: _selected,
        events: events.where((e) => e.occursOn(_selected)).toList(),
      );
      if (isWide(context)) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(child: grid),
            const VerticalDivider(width: 1),
            SizedBox(width: 320, child: agenda),
          ],
        );
      }
      return Column(
        children: [
          SizedBox(height: 380, child: grid),
          const Divider(height: 1),
          Expanded(child: agenda),
        ],
      );
    },
  );

  Widget _header(BuildContext context, {required bool loading}) {
    final forge = Forge.of(context);
    final today = _day(DateTime.now());
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Row(
        children: [
          IconButton(
            tooltip: forge.strings.previousMonth,
            icon: const Icon(Icons.chevron_left),
            onPressed: () =>
                _showMonth(DateTime(_month.year, _month.month - 1)),
          ),
          IconButton(
            tooltip: forge.strings.nextMonth,
            icon: const Icon(Icons.chevron_right),
            onPressed: () =>
                _showMonth(DateTime(_month.year, _month.month + 1)),
          ),
          Expanded(
            child: Text(
              DateFormat.yMMMM(forge.locale).format(_month),
              style: Theme.of(context).textTheme.titleMedium,
            ),
          ),
          if (loading)
            const SizedBox.square(
              dimension: 16,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          TextButton(
            onPressed: () => _showMonth(today, select: today),
            child: Text(forge.strings.today),
          ),
        ],
      ),
    );
  }

  Widget _grid(BuildContext context, List<CalendarEvent> events) {
    final forge = Forge.of(context);
    final theme = Theme.of(context);
    final wide = isWide(context);
    final today = _day(DateTime.now());
    final start = _gridStart;
    final weekdays = DateFormat.E(forge.locale);
    return Column(
      children: [
        Row(
          children: [
            for (var i = 0; i < 7; i++)
              Expanded(
                child: Center(
                  child: Text(
                    weekdays.format(start.add(Duration(days: i))),
                    style: theme.textTheme.labelSmall,
                  ),
                ),
              ),
          ],
        ),
        for (var week = 0; week < 6; week++)
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (var i = 0; i < 7; i++)
                  Expanded(
                    child: _cell(
                      context,
                      DateTime(
                        start.year,
                        start.month,
                        start.day + week * 7 + i,
                      ),
                      events,
                      today: today,
                      wide: wide,
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }

  Widget _cell(
    BuildContext context,
    DateTime day,
    List<CalendarEvent> events, {
    required DateTime today,
    required bool wide,
  }) {
    final forge = Forge.of(context);
    final colors = Theme.of(context).colorScheme;
    final dayEvents = events.where((e) => e.occursOn(day)).toList();
    final selected = day == _selected;
    final inMonth = day.month == _month.month;
    const shown = 3;
    return InkWell(
      onTap: () => setState(() => _selected = day),
      child: Container(
        margin: const EdgeInsets.all(1),
        padding: const EdgeInsets.all(2),
        decoration: BoxDecoration(
          color: selected ? colors.secondaryContainer : null,
          border: Border.all(color: colors.outlineVariant, width: 0.5),
        ),
        // Le contenu qui dépasse de la case est coupé.
        child: SingleChildScrollView(
          physics: const NeverScrollableScrollPhysics(),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                '${day.day}',
                style: TextStyle(
                  fontWeight: day == today ? FontWeight.bold : null,
                  color: day == today
                      ? colors.primary
                      : inMonth
                      ? null
                      : colors.outline,
                ),
              ),
              if (wide)
                for (final event in dayEvents.take(shown))
                  Container(
                    margin: const EdgeInsets.only(top: 1),
                    padding: const EdgeInsets.symmetric(horizontal: 2),
                    color: colors.primaryContainer,
                    child: Text(
                      forge.format.title(widget.table, event.record),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 11,
                        color: colors.onPrimaryContainer,
                      ),
                    ),
                  )
              else if (dayEvents.isNotEmpty)
                Icon(Icons.circle, size: 6, color: colors.primary),
              if (wide && dayEvents.length > shown)
                Text(
                  '+${dayEvents.length - shown}',
                  style: const TextStyle(fontSize: 11),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Agenda extends StatelessWidget {
  const _Agenda({required this.table, required this.day, required this.events});

  final TableSchema table;
  final DateTime day;
  final List<CalendarEvent> events;

  /// Valeur initiale du début pour une création ce jour-là (9 h pour une date-heure).
  String _startValue() {
    final start = table.column(table.calendar!.start)!;
    return start.type == ColumnType.date
        ? dateToJson(day)!
        : dateTimeToJson(DateTime(day.year, day.month, day.day, 9))!;
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final format = forge.format;
    String time(CalendarEvent event) {
      if (event.allDay) return forge.strings.allDay;
      final end = event.end;
      return end == null
          ? format.time(event.start)
          : '${format.time(event.start)} – ${format.time(end)}';
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ListTile(
          title: Text(
            DateFormat.MMMMEEEEd(forge.locale).format(day),
            style: Theme.of(context).textTheme.titleMedium,
          ),
          trailing: forge.can(table, Operation.create)
              ? IconButton(
                  tooltip: forge.strings.create,
                  icon: const Icon(Icons.add),
                  onPressed: () => context.push(
                    Paths.create(table.name, {
                      table.calendar!.start: _startValue(),
                    }),
                  ),
                )
              : null,
        ),
        Expanded(
          child: events.isEmpty
              ? Center(child: Text(forge.strings.noEvents))
              : ListView(
                  children: [
                    for (final event in events)
                      ListTile(
                        leading: Text(time(event)),
                        title: Text(format.title(table, event.record)),
                        onTap: () => context.push(
                          Paths.record(table.name, event.record['id'] as int),
                        ),
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}
