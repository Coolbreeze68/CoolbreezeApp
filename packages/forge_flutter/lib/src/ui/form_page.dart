import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/client.dart';
import '../customization.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import 'fields.dart';
import 'shell.dart';
import 'widgets.dart';

/// Création (`id` absent) ou modification d'un enregistrement.
class FormPage extends StatefulWidget {
  const FormPage({
    super.key,
    required this.table,
    this.id,
    this.initial = const {},
    this.returnsId = false,
  });

  final TableSchema table;
  final int? id;

  /// Valeurs initiales d'une création, au format de l'API (paramètres d'URL).
  final Map<String, String> initial;

  /// Après création, revient à la page précédente avec l'identifiant créé.
  final bool returnsId;

  @override
  State<FormPage> createState() => _FormPageState();
}

class _FormPageState extends State<FormPage> {
  /// Valeurs de départ (enregistrement lu, ou défauts) et valeurs saisies.
  Json? _original;
  final Json _values = {};
  Object? _loadError;
  Map<String, List<String>> _serverErrors = const {};
  bool _submitted = false;
  bool _saving = false;

  TableSchema get table => widget.table;

  bool get _creating => widget.id == null;

  bool get _dirty => _changes().isNotEmpty;

  @override
  void initState() {
    super.initState();
    if (_creating) {
      _original = {
        for (final column in table.editableColumns)
          column.name: switch (widget.initial[column.name]) {
            final text? => parseInitialValue(column, text),
            null => column.defaultValue,
          },
      };
      _values.addAll(_original!);
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_creating && _original == null && _loadError == null) _load();
  }

  Future<void> _load() async {
    try {
      final record = await Forge.of(
        context,
      ).client.table(table.name).read(widget.id!);
      if (!mounted) return;
      setState(() {
        _loadError = null;
        _original = {
          for (final column in table.editableColumns)
            column.name: record[column.name],
        };
        _values
          ..clear()
          ..addAll(_original!);
      });
    } on ApiException catch (error) {
      if (mounted) setState(() => _loadError = error);
    }
  }

  /// Colonnes modifiées : toutes les valeurs renseignées pour une création.
  Json _changes() {
    final original = _original ?? const {};
    return {
      for (final MapEntry(:key, :value) in _values.entries)
        if (_creating ? !isEmptyValue(value) : !_same(value, original[key]))
          key: value,
    };
  }

  static bool _same(Object? a, Object? b) =>
      a is List && b is List ? listEquals(a, b) : a == b;

  Future<void> _save() async {
    final forge = Forge.of(context);
    final s = forge.strings;
    setState(() => _submitted = true);
    final invalid = table.editableColumns.any(
      (c) => clientError(forge, c, _values[c.name]) != null,
    );
    if (invalid) {
      showMessage(context, s.fixErrors);
      return;
    }
    final changes = _changes();
    if (changes.isEmpty && !_creating) return _leave(widget.id!);
    setState(() => _saving = true);
    try {
      final records = forge.client.table(table.name);
      final saved = _creating
          ? await records.create(changes)
          : await records.update(widget.id!, changes);
      forge.changes.notify();
      if (mounted) _afterRebuild(() => _leave(saved['id'] as int));
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() => _serverErrors = error.fields);
      if (error.fields.isEmpty) {
        showError(context, error);
      } else {
        showMessage(context, s.fixErrors);
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  /// Après l'enregistrement : la fiche, en remplaçant le formulaire.
  void _leave(int id) {
    if (widget.returnsId) {
      context.pop(id);
    } else if (_creating || !context.canPop()) {
      context.pushReplacement(Paths.record(table.name, id));
    } else {
      context.pop();
    }
  }

  Future<void> _confirmLeave() async {
    final s = Forge.of(context).strings;
    if (await confirm(context, s.discardChanges, action: s.discard) &&
        mounted) {
      _afterRebuild(context.pop);
    }
  }

  /// Marque le formulaire comme non modifié, puis exécute `action` une fois
  /// la page reconstruite (le blocage du retour arrière est alors levé).
  void _afterRebuild(VoidCallback action) {
    setState(() => _original = {..._values});
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) action();
    });
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final title = _creating
        ? s.newRecord(forge.format.tableLabel(table))
        : s.editRecord(forge.format.tableLabel(table));
    final Widget body;
    if (_loadError case final error?) {
      body = ErrorView(error, onRetry: _load);
    } else if (_original == null) {
      body = const Center(child: CircularProgressIndicator());
    } else {
      // Erreurs du serveur sur des colonnes absentes du formulaire.
      final columns = {for (final c in table.editableColumns) c.name};
      final other = [
        for (final MapEntry(:key, :value) in _serverErrors.entries)
          if (!columns.contains(key)) '$key : ${value.join(', ')}',
      ];
      body = SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: FieldList(
          fields: [
            for (final message in other)
              Text(
                message,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            for (final column in table.editableColumns)
              buildField(
                context,
                FieldState(
                  table: table,
                  column: column,
                  value: _values[column.name],
                  error:
                      (_submitted
                          ? clientError(forge, column, _values[column.name])
                          : null) ??
                      _serverErrors[column.name]?.join('\n'),
                  onChanged: (value) => setState(() {
                    _values[column.name] = value;
                    _serverErrors = {..._serverErrors}..remove(column.name);
                  }),
                ),
              ),
          ],
        ),
      );
    }
    return PopScope(
      canPop: !_dirty,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _confirmLeave();
      },
      child: ForgeScaffold(
        title: Text(title),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 8),
            child: FilledButton.icon(
              onPressed: _saving || _original == null ? null : _save,
              icon: const Icon(Icons.check),
              label: Text(s.save),
            ),
          ),
        ],
        body: body,
      ),
    );
  }
}
