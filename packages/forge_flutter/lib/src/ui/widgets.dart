/// Widgets partagés par les pages.
library;

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/client.dart';
import '../forge.dart';
import '../palette.dart';
import '../router.dart';
import '../schema.dart';
import '../values.dart';

class NotFound extends StatelessWidget {
  const NotFound({super.key});

  @override
  Widget build(BuildContext context) =>
      Center(child: Text(Forge.of(context).strings.notFound));
}

/// Message d'une erreur, lisible par l'utilisateur.
String errorMessage(BuildContext context, Object error) {
  final s = Forge.of(context).strings;
  return switch (error) {
    ApiException(status: 0) => s.networkError,
    ApiException(status: 403) => s.forbidden,
    ApiException(status: 404) => s.notFound,
    ApiException(:final message) when message.isNotEmpty => message,
    _ => s.unexpectedError,
  };
}

void showError(BuildContext context, Object error) {
  ScaffoldMessenger.of(
    context,
  ).showSnackBar(SnackBar(content: Text(errorMessage(context, error))));
}

void showMessage(BuildContext context, String message) {
  ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
}

class ErrorView extends StatelessWidget {
  const ErrorView(this.error, {super.key, this.onRetry});

  final Object error;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.error_outline, color: Theme.of(context).colorScheme.error),
          const SizedBox(height: 8),
          Text(errorMessage(context, error), textAlign: TextAlign.center),
          if (onRetry != null) ...[
            const SizedBox(height: 8),
            TextButton(
              onPressed: onRetry,
              child: Text(Forge.of(context).strings.retry),
            ),
          ],
        ],
      ),
    ),
  );
}

Future<bool> confirm(
  BuildContext context,
  String message, {
  String? action,
}) async {
  final s = Forge.of(context).strings;
  final result = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      content: Text(message),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: Text(s.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: Text(action ?? s.confirm),
        ),
      ],
    ),
  );
  return result ?? false;
}

/// Intitulé d'un enregistrement référencé, chargé par lots.
class RecordTitle extends StatelessWidget {
  const RecordTitle(this.table, this.id, {super.key, this.style});

  final String table;
  final int id;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) => FutureBuilder(
    future: Forge.of(context).titles.title(table, id),
    builder: (context, snapshot) => Text(
      snapshot.data ?? '#$id',
      style: style,
      overflow: TextOverflow.ellipsis,
    ),
  );
}

/// Valeur d'une colonne : texte mis en forme, références cliquables, ou
/// affichage personnalisé (`ForgeCustomization.cells`).
class ValueView extends StatelessWidget {
  const ValueView({
    super.key,
    required this.table,
    required this.column,
    required this.record,
    this.links = true,
    this.maxLines,
  });

  final TableSchema table;
  final ColumnSchema column;
  final Json record;

  /// Références cliquables (désactivé dans les cellules de liste).
  final bool links;
  final int? maxLines;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final value = record[column.name];
    final custom = forge.customization.cells['${table.name}.${column.name}'];
    if (custom != null) return custom(context, record, value);
    final target = column.target;
    if (value is int && column.type == ColumnType.reference && target != null) {
      return _reference(context, target, value);
    }
    if (column.type == ColumnType.referenceList && target != null) {
      final ids = jsonToIds(value);
      if (!links) {
        return Text(ids.isEmpty ? '' : '${ids.length}', maxLines: maxLines);
      }
      return Wrap(
        spacing: 4,
        runSpacing: 4,
        children: [
          for (final id in ids)
            ActionChip(
              label: RecordTitle(target, id),
              onPressed: () => context.push(Paths.record(target, id)),
            ),
        ],
      );
    }
    if (column.type == ColumnType.enumeration && value is String) {
      return Align(
        alignment: AlignmentDirectional.centerStart,
        widthFactor: 1,
        child: EnumBadge(table: table, column: column, value: value),
      );
    }
    if (column.type == ColumnType.boolean && value is bool) {
      return Icon(
        value ? Icons.check_box : Icons.check_box_outline_blank,
        size: 20,
        semanticLabel: forge.format.format(table, column, value),
      );
    }
    return Text(
      forge.format.format(table, column, value),
      maxLines: maxLines,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
    );
  }

  Widget _reference(BuildContext context, String target, int id) {
    final forge = Forge.of(context);
    final readable =
        forge.schema.table(target)?.allows(forge.roles, Operation.read) ??
        false;
    if (!links || !readable) return RecordTitle(target, id);
    return InkWell(
      onTap: () => context.push(Paths.record(target, id)),
      child: RecordTitle(
        target,
        id,
        style: TextStyle(
          color: Theme.of(context).colorScheme.primary,
          decoration: TextDecoration.underline,
        ),
      ),
    );
  }
}

/// Valeur d'énumération : pastille de couleur et libellé (texte neutre).
class EnumBadge extends StatelessWidget {
  const EnumBadge({
    super.key,
    required this.table,
    required this.column,
    required this.value,
  });

  final TableSchema table;
  final ColumnSchema column;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = enumColor(column.values, value, theme.brightness);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: theme.colorScheme.outlineVariant),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 6),
          Text(
            Forge.of(context).format.enumLabel(table, column, value),
            style: theme.textTheme.labelMedium,
          ),
        ],
      ),
    );
  }
}

/// Initiales d'un intitulé, pour l'avatar des tuiles.
String initials(String title) {
  final words = title.split(RegExp(r'\s+')).where((w) => w.isNotEmpty);
  final letters = words.take(2).map((w) => w.characters.first.toUpperCase());
  return letters.isEmpty ? '#' : letters.join();
}

/// Navigation entre les pages d'une liste : « 26–50 sur 112 ».
class Pager extends StatelessWidget {
  const Pager({
    super.key,
    required this.page,
    required this.pages,
    required this.first,
    required this.last,
    required this.total,
    required this.onPage,
  });

  final int page;
  final int pages;
  final int first;
  final int last;
  final int total;
  final ValueChanged<int> onPage;

  @override
  Widget build(BuildContext context) {
    final s = Forge.of(context).strings;
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Text(s.range(first, last, total)),
        IconButton(
          tooltip: s.previousPage,
          onPressed: page > 1 ? () => onPage(page - 1) : null,
          icon: const Icon(Icons.chevron_left),
        ),
        IconButton(
          tooltip: s.nextPage,
          onPressed: page < pages ? () => onPage(page + 1) : null,
          icon: const Icon(Icons.chevron_right),
        ),
      ],
    );
  }
}

/// Largeur à partir de laquelle les pages affichent tableaux et menu fixe.
const wideLayout = 840.0;

bool isWide(BuildContext context) =>
    MediaQuery.sizeOf(context).width >= wideLayout;

/// Contenu centré, de largeur limitée (formulaires, détails).
class Constrained extends StatelessWidget {
  const Constrained({super.key, required this.child, this.maxWidth = 760});

  final Widget child;
  final double maxWidth;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.topCenter,
    child: ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth),
      child: child,
    ),
  );
}
