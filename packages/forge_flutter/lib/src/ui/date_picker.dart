/// Sélecteur de date ou de date-heure : calendrier, heure et raccourcis dans
/// une seule fenêtre.
library;

import 'package:flutter/material.dart';

import '../forge.dart';
import '../values.dart';

/// Ouvre le sélecteur ; renvoie la date choisie (à minuit si `withTime` est
/// faux, en heure locale sinon), ou `null` si l'utilisateur annule.
Future<DateTime?> showForgeDatePicker(
  BuildContext context, {
  DateTime? initial,
  required bool withTime,
}) => showDialog<DateTime>(
  context: context,
  builder: (_) => _DatePickerDialog(initial: initial, withTime: withTime),
);

/// Heure proposée pour une date-heure encore vide.
const _defaultTime = TimeOfDay(hour: DatePreset.morningHour, minute: 0);

class _DatePickerDialog extends StatefulWidget {
  const _DatePickerDialog({required this.initial, required this.withTime});

  final DateTime? initial;
  final bool withTime;

  @override
  State<_DatePickerDialog> createState() => _DatePickerDialogState();
}

class _DatePickerDialogState extends State<_DatePickerDialog> {
  late DateTime _date = widget.initial ?? DateTime.now();
  late TimeOfDay _time = widget.initial == null
      ? _defaultTime
      : TimeOfDay.fromDateTime(widget.initial!);

  DateTime get _value => widget.withTime
      ? DateTime(_date.year, _date.month, _date.day, _time.hour, _time.minute)
      : DateTime(_date.year, _date.month, _date.day);

  Future<void> _pickTime() async {
    final time = await showTimePicker(context: context, initialTime: _time);
    if (time != null) setState(() => _time = time);
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final strings = forge.strings;
    final materialStrings = MaterialLocalizations.of(context);
    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 360),
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(12, 16, 12, 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final preset in DatePreset.of(withTime: widget.withTime))
                    ActionChip(
                      label: Text(preset.label(strings)),
                      onPressed: () => Navigator.pop(
                        context,
                        preset.valueAt(DateTime.now()),
                      ),
                    ),
                ],
              ),
              CalendarDatePicker(
                initialDate: _date,
                firstDate: DateTime(1900),
                lastDate: DateTime(2200),
                onDateChanged: (date) => setState(() => _date = date),
              ),
              if (widget.withTime)
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: OutlinedButton.icon(
                    icon: const Icon(Icons.schedule),
                    label: Text(materialStrings.formatTimeOfDay(_time)),
                    onPressed: _pickTime,
                  ),
                ),
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.pop(context),
                    child: Text(strings.cancel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: () => Navigator.pop(context, _value),
                    child: Text(strings.ok),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
