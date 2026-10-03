/// Champs de formulaire par type de colonne. Chaque champ manipule la valeur au
/// format JSON de l'API ; une saisie illisible est signalée par [InvalidInput].
library;

import 'package:decimal/decimal.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../customization.dart';
import '../forge.dart';
import '../schema.dart';
import '../values.dart';
import 'field_models.dart';
import 'reference_picker.dart';
import 'widgets.dart';

/// Saisie qui ne correspond pas au type de la colonne (nombre mal écrit…).
class InvalidInput {
  const InvalidInput(this.text);

  final String text;
}

/// Champ d'une colonne : personnalisé (`ForgeCustomization.fields`) ou par défaut.
Widget buildField(BuildContext context, FieldState field) {
  final custom = Forge.of(
    context,
  ).customization.fields['${field.table.name}.${field.column.name}'];
  if (custom != null) return custom(context, field);
  return switch (field.column.type) {
    ColumnType.boolean => _BoolField(field),
    ColumnType.date || ColumnType.datetime => _DateField(field),
    ColumnType.enumeration => _EnumField(field),
    ColumnType.reference => _ReferenceField(field),
    ColumnType.referenceList => _ReferenceListField(field),
    ColumnType.color => ColorField(field),
    ColumnType.rating => RatingField(field),
    ColumnType.markdown => MarkdownField(field),
    ColumnType.file || ColumnType.image => FileField(field),
    _ => _TextInputField(field),
  };
}

const _decoration = fieldDecoration;

/// Saisies au clavier : texte (dont e-mail, adresse web, téléphone), nombres,
/// pourcentages, montants et durées (`h:mm`, ou un nombre de minutes).
class _TextInputField extends StatefulWidget {
  const _TextInputField(this.field);

  final FieldState field;

  @override
  State<_TextInputField> createState() => _TextInputFieldState();
}

class _TextInputFieldState extends State<_TextInputField> {
  late final _controller = TextEditingController(
    text: _text(widget.field.value),
  );

  ColumnType get _type => widget.field.column.type;

  String _text(Object? value) => switch ((value, _type)) {
    (null, _) => '',
    (InvalidInput(:final text), _) => text,
    (final int seconds, ColumnType.duration) =>
      '${seconds ~/ 3600}:${(seconds % 3600 ~/ 60).toString().padLeft(2, '0')}',
    (final v, ColumnType.percent) => switch (jsonToDecimal(v)) {
      final ratio? => (ratio * Decimal.fromInt(100)).toString(),
      null => '$v',
    },
    (final v, _) => '$v',
  };

  Object? _parse(String text) {
    if (text.trim().isEmpty) return null;
    final parsed = switch (_type) {
      ColumnType.integer => parseIntegerInput(text),
      ColumnType.decimal => parseDecimalInput(text),
      ColumnType.duration => _parseDuration(text.trim()),
      ColumnType.money => parseDecimalInput(text),
      ColumnType.percent => parsePercentInput(text),
      ColumnType.email ||
      ColumnType.url ||
      ColumnType.phone => isValidInput(_type, text.trim()) ? text.trim() : null,
      _ => text,
    };
    return parsed ?? InvalidInput(text);
  }

  static int? _parseDuration(String text) {
    final match = RegExp(r'^(\d+)(?::(\d{1,2}))?$').firstMatch(text);
    if (match == null) return null;
    final first = int.parse(match[1]!);
    return match[2] == null
        ? first * 60
        : first * 3600 + int.parse(match[2]!) * 60;
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final multiline = _type == ColumnType.text;
    return TextField(
      controller: _controller,
      decoration: _decoration(context, widget.field).copyWith(
        helperText: _type == ColumnType.duration
            ? forge.strings.durationHint
            : null,
        suffixText: switch (_type) {
          ColumnType.percent => '%',
          ColumnType.money => NumberFormat.simpleCurrency(
            locale: forge.locale,
            name: widget.field.column.currency,
          ).currencySymbol,
          _ => null,
        },
        prefixIcon: switch (_type) {
          ColumnType.email => const Icon(Icons.mail_outline),
          ColumnType.url => const Icon(Icons.link),
          ColumnType.phone => const Icon(Icons.phone_outlined),
          _ => null,
        },
      ),
      keyboardType: switch (_type) {
        ColumnType.integer => const TextInputType.numberWithOptions(
          signed: true,
        ),
        ColumnType.decimal || ColumnType.percent || ColumnType.money =>
          const TextInputType.numberWithOptions(signed: true, decimal: true),
        ColumnType.duration => TextInputType.datetime,
        ColumnType.email => TextInputType.emailAddress,
        ColumnType.url => TextInputType.url,
        ColumnType.phone => TextInputType.phone,
        _ when multiline => TextInputType.multiline,
        _ => TextInputType.text,
      },
      minLines: multiline ? 3 : 1,
      maxLines: multiline ? 8 : 1,
      maxLength: switch (_type) {
        ColumnType.string ||
        ColumnType.email ||
        ColumnType.url ||
        ColumnType.phone => 255,
        _ => null,
      },
      buildCounter: _hideCounter,
      onChanged: (text) => widget.field.onChanged(_parse(text)),
    );
  }

  static Widget? _hideCounter(
    BuildContext context, {
    required int currentLength,
    required bool isFocused,
    required int? maxLength,
  }) => null;
}

class _BoolField extends StatelessWidget {
  const _BoolField(this.field);

  final FieldState field;

  @override
  Widget build(BuildContext context) => InputDecorator(
    decoration: _decoration(
      context,
      field,
    ).copyWith(border: InputBorder.none, contentPadding: EdgeInsets.zero),
    child: Align(
      alignment: Alignment.centerLeft,
      child: Switch(value: field.value == true, onChanged: field.onChanged),
    ),
  );
}

/// Date (`2026-10-02`) ou date-heure (RFC 3339, saisie en heure locale).
class _DateField extends StatelessWidget {
  const _DateField(this.field);

  final FieldState field;

  bool get _withTime => field.column.type == ColumnType.datetime;

  Future<void> _pick(BuildContext context) async {
    final current = _withTime
        ? jsonToDateTime(field.value)
        : jsonToDate(field.value);
    final date = await showDatePicker(
      context: context,
      initialDate: current ?? DateTime.now(),
      firstDate: DateTime(1900),
      lastDate: DateTime(2200),
    );
    if (date == null || !context.mounted) return;
    if (!_withTime) return field.onChanged(dateToJson(date));
    final time = await showTimePicker(
      context: context,
      initialTime: TimeOfDay.fromDateTime(current ?? DateTime(0, 1, 1, 9)),
    );
    if (time == null) return;
    field.onChanged(
      dateTimeToJson(
        DateTime(date.year, date.month, date.day, time.hour, time.minute),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final format = Forge.of(context).format;
    final value = _withTime
        ? jsonToDateTime(field.value)
        : jsonToDate(field.value);
    return InkWell(
      onTap: () => _pick(context),
      child: InputDecorator(
        decoration: _decoration(context, field).copyWith(
          suffixIcon: value == null
              ? const Icon(Icons.event)
              : IconButton(
                  tooltip: Forge.of(context).strings.clear,
                  icon: const Icon(Icons.clear),
                  onPressed: () => field.onChanged(null),
                ),
        ),
        isEmpty: value == null,
        child: Text(
          value == null
              ? ''
              : _withTime
              ? format.dateTime(value)
              : format.date(value),
        ),
      ),
    );
  }
}

class _EnumField extends StatelessWidget {
  const _EnumField(this.field);

  final FieldState field;

  @override
  Widget build(BuildContext context) {
    final format = Forge.of(context).format;
    final current = field.value is String ? field.value as String : null;
    return DropdownButtonFormField<String?>(
      key: ValueKey(current),
      initialValue: current,
      decoration: _decoration(context, field),
      items: [
        if (!field.column.required)
          const DropdownMenuItem(value: null, child: Text('')),
        for (final value in field.column.values)
          DropdownMenuItem(
            value: value,
            child: Text(format.enumLabel(field.table, field.column, value)),
          ),
      ],
      onChanged: field.onChanged,
    );
  }
}

class _ReferenceField extends StatelessWidget {
  const _ReferenceField(this.field);

  final FieldState field;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final target = forge.schema.table(field.column.target!)!;
    final id = field.value is int ? field.value as int : null;
    return InkWell(
      onTap: () async {
        final ids = await pickRecords(context, target, selected: [?id]);
        if (ids != null && ids.isNotEmpty) field.onChanged(ids.first);
      },
      child: InputDecorator(
        decoration: _decoration(context, field).copyWith(
          suffixIcon: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              CreateReferenceButton(table: target, onCreated: field.onChanged),
              if (id == null || field.column.required)
                const Padding(
                  padding: EdgeInsets.only(right: 12),
                  child: Icon(Icons.arrow_drop_down),
                )
              else
                IconButton(
                  tooltip: forge.strings.clear,
                  icon: const Icon(Icons.clear),
                  onPressed: () => field.onChanged(null),
                ),
            ],
          ),
        ),
        isEmpty: id == null,
        child: id == null ? const Text('') : RecordTitle(target.name, id),
      ),
    );
  }
}

class _ReferenceListField extends StatelessWidget {
  const _ReferenceListField(this.field);

  final FieldState field;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final target = forge.schema.table(field.column.target!)!;
    final ids = jsonToIds(field.value);
    return InputDecorator(
      decoration: _decoration(context, field),
      child: Wrap(
        spacing: 6,
        runSpacing: 6,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          for (final id in ids)
            InputChip(
              label: RecordTitle(target.name, id),
              onDeleted: () =>
                  field.onChanged([...ids.where((other) => other != id)]),
            ),
          ActionChip(
            avatar: const Icon(Icons.add, size: 18),
            label: Text(forge.strings.add),
            onPressed: () async {
              final picked = await pickRecords(
                context,
                target,
                multiple: true,
                selected: ids,
              );
              if (picked != null) field.onChanged(picked);
            },
          ),
          CreateReferenceButton(
            table: target,
            onCreated: (id) => field.onChanged([...ids, id]),
          ),
        ],
      ),
    );
  }
}

/// Valeur d'URL (format de l'API, en texte) convertie au JSON de la colonne.
Object? parseInitialValue(ColumnSchema column, String text) =>
    switch (column.type) {
      ColumnType.integer ||
      ColumnType.duration ||
      ColumnType.rating ||
      ColumnType.reference => int.tryParse(text),
      ColumnType.boolean => text == 'true',
      ColumnType.referenceList => [
        for (final part in text.split(',')) ?int.tryParse(part),
      ],
      _ => text,
    };

/// Valeur absente : `null`, texte vide ou liste vide.
bool isEmptyValue(Object? value) =>
    value == null || value == '' || (value is List && value.isEmpty);

/// Erreur affichée sous un champ requis vide ou mal saisi.
String? clientError(Forge forge, ColumnSchema column, Object? value) {
  if (value is InvalidInput) return forge.strings.invalidValue;
  if (column.required && isEmptyValue(value)) return forge.strings.required;
  return null;
}

/// Champs d'un formulaire, l'un sous l'autre.
class FieldList extends StatelessWidget {
  const FieldList({super.key, required this.fields});

  final List<Widget> fields;

  @override
  Widget build(BuildContext context) => Constrained(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final field in fields)
          Padding(padding: const EdgeInsets.only(bottom: 16), child: field),
      ],
    ),
  );
}
