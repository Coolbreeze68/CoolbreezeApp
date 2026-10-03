import 'package:decimal/decimal.dart';
import 'package:flutter/material.dart';

import '../api/query.dart';
import '../forge.dart';
import '../schema.dart';
import '../values.dart';
import 'reference_picker.dart';
import 'widgets.dart';

/// Filtre posé par l'utilisateur sur une colonne, traduit en filtres d'API.
sealed class ColumnFilter {
  const ColumnFilter(this.column);

  final ColumnSchema column;

  List<Filter> get filters;

  /// Filtre d'égalité lu dans l'URL (`?entreprise=3`), au format de l'API.
  static ColumnFilter? fromParameter(ColumnSchema column, String value) =>
      switch (column.type) {
        ColumnType.reference => switch (int.tryParse(value)) {
          final id? => ReferenceFilter(column, id),
          null => null,
        },
        ColumnType.enumeration => ChoiceFilter(column, {value}),
        ColumnType.boolean => BoolFilter(column, value == 'true'),
        _ => EqualsFilter(column, value),
      };
}

/// Énumération : une valeur parmi plusieurs.
class ChoiceFilter extends ColumnFilter {
  const ChoiceFilter(super.column, this.values);

  final Set<String> values;

  @override
  List<Filter> get filters => [Filter.oneOf(column.name, values)];
}

class BoolFilter extends ColumnFilter {
  const BoolFilter(super.column, this.value);

  final bool value;

  @override
  List<Filter> get filters => [Filter.equals(column.name, '$value')];
}

/// Texte contenu, sans tenir compte de la casse.
class TextFilter extends ColumnFilter {
  const TextFilter(super.column, this.text);

  final String text;

  @override
  List<Filter> get filters => [Filter(column.name, FilterOp.like, text)];
}

class ReferenceFilter extends ColumnFilter {
  const ReferenceFilter(super.column, this.id);

  final int id;

  @override
  List<Filter> get filters => [Filter.equals(column.name, '$id')];
}

class EqualsFilter extends ColumnFilter {
  const EqualsFilter(super.column, this.value);

  final String value;

  @override
  List<Filter> get filters => [Filter.equals(column.name, value)];
}

/// Intervalle, bornes incluses, au format JSON de la colonne ; pour une
/// date-heure, bornes en jours (`2026-10-02`) couvrant les journées entières.
class RangeFilter extends ColumnFilter {
  const RangeFilter(super.column, {this.min, this.max});

  final Object? min;
  final Object? max;

  @override
  List<Filter> get filters {
    if (column.type == ColumnType.datetime) {
      final from = jsonToDate(min);
      final to = jsonToDate(max);
      return [
        if (from != null)
          Filter(column.name, FilterOp.gte, dateTimeToJson(from)!),
        if (to != null)
          Filter(
            column.name,
            FilterOp.lt,
            dateTimeToJson(DateTime(to.year, to.month, to.day + 1))!,
          ),
      ];
    }
    return [
      if (min != null) Filter(column.name, FilterOp.gte, '$min'),
      if (max != null) Filter(column.name, FilterOp.lte, '$max'),
    ];
  }
}

/// Colonnes filtrables : stockées et affichées.
Iterable<ColumnSchema> filterableColumns(TableSchema table) =>
    table.visibleColumns.where(
      (c) => c.stored && c.type != ColumnType.referenceList && !c.type.isFile,
    );

/// Colonne filtrée par « contient ».
bool _isText(ColumnType type) => switch (type) {
  ColumnType.string ||
  ColumnType.text ||
  ColumnType.color ||
  ColumnType.email ||
  ColumnType.url ||
  ColumnType.phone ||
  ColumnType.markdown => true,
  _ => false,
};

/// Puce d'un filtre actif : « Étape : Gagné, Perdu ».
class FilterChipView extends StatelessWidget {
  const FilterChipView({
    super.key,
    required this.table,
    required this.filter,
    required this.onEdit,
    required this.onDeleted,
  });

  final TableSchema table;
  final ColumnFilter filter;
  final VoidCallback onEdit;
  final VoidCallback onDeleted;

  @override
  Widget build(BuildContext context) {
    final format = Forge.of(context).format;
    final column = filter.column;
    String value(Object? json) => format.format(
      table,
      column.type == ColumnType.datetime
          ? const ColumnSchema('', ColumnType.date)
          : column,
      json,
    );
    final Widget description = switch (filter) {
      ChoiceFilter(:final values) => Text(
        values.map((v) => format.enumLabel(table, column, v)).join(', '),
      ),
      BoolFilter(value: final b) => Text(
        b ? format.strings.yes : format.strings.no,
      ),
      TextFilter(:final text) => Text('« $text »'),
      ReferenceFilter(:final id) => RecordTitle(column.target!, id),
      EqualsFilter(value: final v) => Text(v),
      RangeFilter(:final min, :final max) => Text(switch ((min, max)) {
        (final a?, final b?) => '${value(a)} – ${value(b)}',
        (final a?, null) => '≥ ${value(a)}',
        (null, final b?) => '≤ ${value(b)}',
        _ => '',
      }),
    };
    return InputChip(
      label: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text('${format.columnLabel(column)} : '),
          Flexible(child: description),
        ],
      ),
      onPressed: onEdit,
      onDeleted: onDeleted,
    );
  }
}

/// Édite le filtre d'une colonne : `filter` est le nouveau filtre (`null` pour
/// le retirer) ; si l'utilisateur annule, `cancelled` est vrai.
Future<({ColumnFilter? filter, bool cancelled})> editFilter(
  BuildContext context,
  TableSchema table,
  ColumnSchema column,
  ColumnFilter? current,
) async {
  if (column.type == ColumnType.reference) {
    final target = Forge.of(context).schema.table(column.target!)!;
    final ids = await pickRecords(
      context,
      target,
      selected: [if (current case ReferenceFilter(:final id)) id],
    );
    if (ids == null) return (filter: current, cancelled: true);
    return (
      filter: ids.isEmpty ? null : ReferenceFilter(column, ids.first),
      cancelled: false,
    );
  }
  final result = await showDialog<_Edited>(
    context: context,
    builder: (_) =>
        _FilterDialog(table: table, column: column, current: current),
  );
  return result == null
      ? (filter: current, cancelled: true)
      : (filter: result.filter, cancelled: false);
}

class _Edited {
  const _Edited(this.filter);

  final ColumnFilter? filter;
}

class _FilterDialog extends StatefulWidget {
  const _FilterDialog({
    required this.table,
    required this.column,
    required this.current,
  });

  final TableSchema table;
  final ColumnSchema column;
  final ColumnFilter? current;

  @override
  State<_FilterDialog> createState() => _FilterDialogState();
}

class _FilterDialogState extends State<_FilterDialog> {
  late Set<String> _choices = switch (widget.current) {
    ChoiceFilter(:final values) => {...values},
    _ => {},
  };
  late bool? _bool = switch (widget.current) {
    BoolFilter(:final value) => value,
    _ => null,
  };
  late final _text = TextEditingController(
    text: switch (widget.current) {
      TextFilter(:final text) => text,
      _ => '',
    },
  );
  late Object? _min = switch (widget.current) {
    RangeFilter(:final min) => min,
    _ => null,
  };
  late Object? _max = switch (widget.current) {
    RangeFilter(:final max) => max,
    _ => null,
  };

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  ColumnSchema get column => widget.column;

  ColumnFilter? _build() => switch (column.type) {
    ColumnType.enumeration =>
      _choices.isEmpty ? null : ChoiceFilter(column, _choices),
    ColumnType.boolean => _bool == null ? null : BoolFilter(column, _bool!),
    final type when _isText(type) =>
      _text.text.trim().isEmpty ? null : TextFilter(column, _text.text.trim()),
    _ =>
      _min == null && _max == null
          ? null
          : RangeFilter(column, min: _min, max: _max),
  };

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    return AlertDialog(
      title: Text(forge.format.columnLabel(column)),
      content: SizedBox(width: 400, child: _editor(forge)),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, const _Edited(null)),
          child: Text(s.clear),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, _Edited(_build())),
          child: Text(s.apply),
        ),
      ],
    );
  }

  Widget _editor(Forge forge) {
    final s = forge.strings;
    switch (column.type) {
      case ColumnType.enumeration:
        return Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final value in column.values)
              FilterChip(
                label: Text(
                  forge.format.enumLabel(widget.table, column, value),
                ),
                selected: _choices.contains(value),
                onSelected: (on) => setState(
                  () => _choices = on
                      ? {..._choices, value}
                      : ({..._choices}..remove(value)),
                ),
              ),
          ],
        );
      case ColumnType.boolean:
        return SegmentedButton<bool?>(
          segments: [
            ButtonSegment(value: null, label: Text(s.all)),
            ButtonSegment(value: true, label: Text(s.yes)),
            ButtonSegment(value: false, label: Text(s.no)),
          ],
          selected: {_bool},
          onSelectionChanged: (value) => setState(() => _bool = value.first),
        );
      case final type when _isText(type):
        return TextField(
          controller: _text,
          autofocus: true,
          decoration: InputDecoration(labelText: s.contains),
          onSubmitted: (_) => Navigator.pop(context, _Edited(_build())),
        );
      case ColumnType.date || ColumnType.datetime:
        return Row(
          children: [
            Expanded(child: _dateBound(s.from, _min, (v) => _min = v)),
            const SizedBox(width: 8),
            Expanded(child: _dateBound(s.to, _max, (v) => _max = v)),
          ],
        );
      case _:
        return Row(
          children: [
            Expanded(child: _numberBound(s.min, _min, (v) => _min = v)),
            const SizedBox(width: 8),
            Expanded(child: _numberBound(s.max, _max, (v) => _max = v)),
          ],
        );
    }
  }

  Widget _dateBound(String label, Object? value, ValueChanged<Object?> set) {
    final format = Forge.of(context).format;
    final date = jsonToDate(value);
    return OutlinedButton(
      onPressed: () async {
        final picked = await showDatePicker(
          context: context,
          initialDate: date ?? DateTime.now(),
          firstDate: DateTime(1900),
          lastDate: DateTime(2200),
        );
        if (picked != null) setState(() => set(dateToJson(picked)));
      },
      child: Text(date == null ? label : '$label ${format.date(date)}'),
    );
  }

  /// Borne numérique ; une durée se saisit en minutes.
  Widget _numberBound(String label, Object? value, ValueChanged<Object?> set) {
    final s = Forge.of(context).strings;
    final minutes = column.type == ColumnType.duration;
    final percent = column.type == ColumnType.percent;
    return TextFormField(
      initialValue: switch (value) {
        null => '',
        final v when minutes => '${(v as num) ~/ 60}',
        final v when percent => switch (jsonToDecimal(v)) {
          final ratio? => (ratio * Decimal.fromInt(100)).toString(),
          null => '$v',
        },
        final v => '$v',
      },
      decoration: InputDecoration(
        labelText: label,
        suffixText: minutes
            ? s.minutesUnit
            : percent
            ? '%'
            : null,
      ),
      keyboardType: const TextInputType.numberWithOptions(
        decimal: true,
        signed: true,
      ),
      onChanged: (text) => set(switch (column.type) {
        ColumnType.decimal || ColumnType.money => parseDecimalInput(text),
        ColumnType.percent => parsePercentInput(text),
        ColumnType.duration => switch (parseIntegerInput(text)) {
          final m? => m * 60,
          null => null,
        },
        _ => parseIntegerInput(text),
      }),
    );
  }
}
