import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/client.dart';
import '../api/query.dart';
import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import '../values.dart';
import 'record_list.dart';
import 'shell.dart';
import 'widgets.dart';

/// Fiche d'un enregistrement : ses valeurs, les blocs personnalisés et les
/// enregistrements d'autres tables qui le référencent.
class DetailPage extends StatefulWidget {
  const DetailPage({super.key, required this.table, required this.id});

  final TableSchema table;
  final int id;

  @override
  State<DetailPage> createState() => _DetailPageState();
}

class _DetailPageState extends State<DetailPage> {
  Future<Json>? _record;
  DataChanges? _changes;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
    }
    _record ??= _load();
  }

  @override
  void dispose() {
    _changes?.removeListener(_reload);
    super.dispose();
  }

  Future<Json> _load() =>
      Forge.of(context).client.table(widget.table.name).read(widget.id);

  void _reload() => setState(() {
    _record = _load();
  });

  Future<void> _delete(Json record) async {
    final forge = Forge.of(context);
    final s = forge.strings;
    final title = forge.format.title(widget.table, record);
    if (!await confirm(context, s.confirmDelete(title), action: s.delete)) {
      return;
    }
    try {
      await forge.client.table(widget.table.name).delete(widget.id);
      // Retirer l'écouteur avant de signaler : la fiche n'existe plus.
      _changes?.removeListener(_reload);
      forge.changes.notify();
      if (!mounted) return;
      showMessage(context, s.deleted);
      if (context.canPop()) {
        context.pop();
      } else {
        context.go(Paths.table(widget.table.name));
      }
    } on ApiException catch (error) {
      if (mounted) showError(context, error);
    }
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final table = widget.table;
    return FutureBuilder(
      future: _record,
      builder: (context, snapshot) {
        final record = snapshot.data;
        return ForgeScaffold(
          title: Text(
            record == null
                ? forge.format.tableLabel(table)
                : forge.format.title(table, record),
          ),
          actions: [
            if (record != null && forge.can(table, Operation.update))
              IconButton(
                tooltip: s.edit,
                icon: const Icon(Icons.edit),
                onPressed: () =>
                    context.push(Paths.edit(table.name, widget.id)),
              ),
            if (record != null && forge.can(table, Operation.delete))
              IconButton(
                tooltip: s.delete,
                icon: const Icon(Icons.delete_outline),
                onPressed: () => _delete(record),
              ),
          ],
          body: switch (snapshot) {
            AsyncSnapshot(:final error?) => ErrorView(error, onRetry: _reload),
            AsyncSnapshot(data: final record?) => _body(context, record),
            _ => const Center(child: CircularProgressIndicator()),
          },
        );
      },
    );
  }

  Widget _body(BuildContext context, Json record) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final table = widget.table;
    final sections = forge.customization.detailSections[table.name] ?? const [];
    final related = [
      for (final list in forge.schema.relatedLists(table))
        if (forge.can(list.table, Operation.read)) list,
    ];
    final created = jsonToDateTime(record['created_at']);
    final updated = jsonToDateTime(record['updated_at']);
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Constrained(
        maxWidth: 960,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final column in table.visibleColumns)
                      _Field(
                        label: forge.format.columnLabel(column),
                        child: ValueView(
                          table: table,
                          column: column,
                          record: record,
                        ),
                      ),
                    if (created != null && updated != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 12),
                        child: Text(
                          s.timestamps(
                            forge.format.dateTime(created),
                            forge.format.dateTime(updated),
                          ),
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                  ],
                ),
              ),
            ),
            for (final section in sections) ...[
              const SizedBox(height: 16),
              section(context, record),
            ],
            for (final list in related) ...[
              const SizedBox(height: 16),
              _RelatedCard(
                list: list,
                id: widget.id,
                // Libellé de colonne ajouté si la table référence deux fois.
                showColumn:
                    related.where((r) => r.table == list.table).length > 1,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _Field extends StatelessWidget {
  const _Field({required this.label, required this.child});

  final String label;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final labelStyle = Theme.of(context).textTheme.labelMedium?.copyWith(
      color: Theme.of(context).colorScheme.onSurfaceVariant,
    );
    final labelText = Text(label, style: labelStyle);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: isWide(context)
          ? Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(width: 200, child: labelText),
                Expanded(child: child),
              ],
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [labelText, const SizedBox(height: 2), child],
            ),
    );
  }
}

/// Enregistrements d'une autre table qui référencent la fiche.
class _RelatedCard extends StatelessWidget {
  const _RelatedCard({
    required this.list,
    required this.id,
    required this.showColumn,
  });

  final RelatedList list;
  final int id;
  final bool showColumn;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final table = list.table;
    final link = {list.column.name: '$id'};
    var title = forge.format.tableLabel(table);
    if (showColumn) title += ' (${forge.format.columnLabel(list.column)})';
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          ListTile(
            title: Text(title, style: Theme.of(context).textTheme.titleMedium),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextButton(
                  onPressed: () => context.push(
                    Uri(
                      path: Paths.table(table.name),
                      queryParameters: link,
                    ).toString(),
                  ),
                  child: Text(s.seeAll),
                ),
                if (forge.can(table, Operation.create))
                  IconButton(
                    tooltip: s.create,
                    icon: const Icon(Icons.add),
                    onPressed: () =>
                        context.push(Paths.create(table.name, link)),
                  ),
              ],
            ),
          ),
          RecordList(
            table: table,
            compact: true,
            query: ListQuery(
              perPage: 5,
              filters: [Filter.equals(list.column.name, '$id')],
            ),
          ),
        ],
      ),
    );
  }
}
