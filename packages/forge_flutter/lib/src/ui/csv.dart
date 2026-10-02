import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../api/client.dart';
import '../api/query.dart';
import '../forge.dart';
import '../schema.dart';
import '../values.dart';
import 'widgets.dart';

/// Exporte les enregistrements correspondant à `query` dans un fichier CSV
/// (séparateur `;` dans les langues où la virgule est décimale, comme le
/// veulent les tableurs).
Future<void> exportCsv(
  BuildContext context,
  TableSchema table,
  ListQuery query,
) async {
  final forge = Forge.of(context);
  final decimalComma =
      NumberFormat.decimalPattern(forge.locale).symbols.DECIMAL_SEP == ',';
  try {
    final bytes = await forge.client
        .table(table.name)
        .export(query: query, delimiter: decimalComma ? ';' : ',');
    final saved = await FilePicker.saveFile(
      fileName: '${table.name}-${dateToJson(DateTime.now())}.csv',
      bytes: bytes,
      mimeType: 'text/csv',
    );
    if (saved != null && context.mounted) {
      showMessage(context, forge.strings.exported);
    }
  } on ApiException catch (error) {
    if (context.mounted) showError(context, error);
  }
}

/// Importe un fichier CSV choisi par l'utilisateur ; tout ou rien.
Future<void> importCsv(BuildContext context, TableSchema table) async {
  final forge = Forge.of(context);
  final s = forge.strings;
  final file = await FilePicker.pickFile(
    type: FileType.custom,
    allowedExtensions: const ['csv'],
  );
  if (file == null) return;
  final bytes = await file.readAsBytes();
  try {
    final report = await forge.client.table(table.name).import(bytes);
    forge.changes.notify();
    if (context.mounted) {
      showMessage(context, s.imported(report.created, report.updated));
    }
  } on ApiException catch (error) {
    if (!context.mounted) return;
    if (error.lines.isEmpty) return showError(context, error);
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(s.importRejected),
        content: SizedBox(
          width: 520,
          child: ListView(
            shrinkWrap: true,
            children: [
              for (final line in error.lines)
                ListTile(
                  dense: true,
                  leading: Text(s.line(line.line)),
                  title: Text(line.message),
                  subtitle: line.fields.isEmpty
                      ? null
                      : Text(
                          [
                            for (final MapEntry(:key, :value)
                                in line.fields.entries)
                              '$key : ${value.join(', ')}',
                          ].join('\n'),
                        ),
                ),
            ],
          ),
        ),
        actions: [
          FilledButton(
            onPressed: () => Navigator.pop(context),
            child: Text(s.ok),
          ),
        ],
      ),
    );
  }
}
