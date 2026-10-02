import 'package:flutter/material.dart';

import '../api/client.dart';
import '../customization.dart';
import '../forge.dart';
import '../schema.dart';
import 'fields.dart';
import 'shell.dart';
import 'widgets.dart';

/// Table fictive portant les paramètres, pour réutiliser les champs.
const _parameters = TableSchema('parameters', columns: []);

/// Paramètres de l'application (administrateurs). Un changement recalcule
/// les formules qui les lisent.
class ParametersPage extends StatefulWidget {
  const ParametersPage({super.key});

  @override
  State<ParametersPage> createState() => _ParametersPageState();
}

class _ParametersPageState extends State<ParametersPage> {
  Future<Map<String, Object?>>? _values;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _values ??= _load();
  }

  Future<Map<String, Object?>> _load() async {
    final list = await Forge.of(context).client.get('/api/parameters') as List;
    return {
      for (final item in list.cast<Json>())
        item['name'] as String: item['value'],
    };
  }

  Future<void> _edit(Parameter parameter, Object? value) async {
    final saved = await showDialog<bool>(
      context: context,
      builder: (_) => _ParameterDialog(parameter: parameter, value: value),
    );
    if (saved == true && mounted) {
      Forge.of(context).changes.notify();
      setState(() {
        _values = _load();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    return ForgeScaffold(
      title: Text(forge.strings.parameters),
      body: FutureBuilder(
        future: _values,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return ErrorView(
              snapshot.error!,
              onRetry: () => setState(() {
                _values = _load();
              }),
            );
          }
          final values = snapshot.data;
          if (values == null) {
            return const Center(child: CircularProgressIndicator());
          }
          return ListView(
            children: [
              for (final parameter in forge.schema.parameters)
                ListTile(
                  title: Text(
                    forge.format.label(parameter.label, parameter.name),
                  ),
                  subtitle: Text(
                    forge.format.format(
                      _parameters,
                      _column(parameter),
                      values[parameter.name],
                    ),
                  ),
                  trailing: const Icon(Icons.edit),
                  onTap: () => _edit(parameter, values[parameter.name]),
                ),
            ],
          );
        },
      ),
    );
  }
}

ColumnSchema _column(Parameter parameter) =>
    ColumnSchema(parameter.name, parameter.type, label: parameter.label);

class _ParameterDialog extends StatefulWidget {
  const _ParameterDialog({required this.parameter, required this.value});

  final Parameter parameter;
  final Object? value;

  @override
  State<_ParameterDialog> createState() => _ParameterDialogState();
}

class _ParameterDialogState extends State<_ParameterDialog> {
  late Object? _value = widget.value;
  String? _error;
  bool _saving = false;

  Future<void> _save() async {
    final forge = Forge.of(context);
    if (_value is InvalidInput) {
      setState(() => _error = forge.strings.invalidValue);
      return;
    }
    setState(() => _saving = true);
    try {
      await forge.client.put('/api/parameters/${widget.parameter.name}', {
        'value': _value,
      });
      if (mounted) Navigator.pop(context, true);
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(
        () => _error = error.fields.values
            .expand((m) => m)
            .join('\n')
            .ifEmpty(errorMessage(context, error)),
      );
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = Forge.of(context).strings;
    return AlertDialog(
      content: SizedBox(
        width: 400,
        child: buildField(
          context,
          FieldState(
            table: _parameters,
            column: _column(widget.parameter),
            value: _value,
            error: _error,
            onChanged: (value) => setState(() {
              _value = value;
              _error = null;
            }),
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: Text(s.cancel),
        ),
        FilledButton(onPressed: _saving ? null : _save, child: Text(s.save)),
      ],
    );
  }
}

extension on String {
  String ifEmpty(String other) => isEmpty ? other : this;
}
