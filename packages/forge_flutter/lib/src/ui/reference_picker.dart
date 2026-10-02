import 'dart:async';

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../api/query.dart';
import '../forge.dart';
import '../schema.dart';
import 'widgets.dart';

/// Choix d'enregistrements de `target`, avec recherche. Retourne les
/// identifiants choisis, ou `null` si l'utilisateur annule.
Future<List<int>?> pickRecords(
  BuildContext context,
  TableSchema target, {
  bool multiple = false,
  List<int> selected = const [],
}) => showDialog<List<int>>(
  context: context,
  builder: (_) =>
      _PickerDialog(target: target, multiple: multiple, selected: selected),
);

class _PickerDialog extends StatefulWidget {
  const _PickerDialog({
    required this.target,
    required this.multiple,
    required this.selected,
  });

  final TableSchema target;
  final bool multiple;
  final List<int> selected;

  @override
  State<_PickerDialog> createState() => _PickerDialogState();
}

class _PickerDialogState extends State<_PickerDialog> {
  late final Set<int> _selected = {...widget.selected};
  var _query = const ListQuery(perPage: 20);
  Future<Listing<Json>>? _listing;
  Timer? _debounce;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _listing ??= _load();
  }

  Future<Listing<Json>> _load() =>
      Forge.of(context).client.table(widget.target.name).list(_query);

  void _search(String text) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () {
      setState(() {
        _query = _query.copyWith(search: text, page: 1);
        _listing = _load();
      });
    });
  }

  @override
  void dispose() {
    _debounce?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    return AlertDialog(
      title: Text(forge.format.tableLabel(widget.target)),
      contentPadding: const EdgeInsets.symmetric(vertical: 8),
      content: SizedBox(
        width: 480,
        height: 480,
        child: Column(
          children: [
            if (widget.target.searchable)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: TextField(
                  autofocus: true,
                  decoration: InputDecoration(
                    prefixIcon: const Icon(Icons.search),
                    hintText: s.search,
                  ),
                  onChanged: _search,
                ),
              ),
            Expanded(
              child: FutureBuilder(
                future: _listing,
                builder: (context, snapshot) {
                  if (snapshot.hasError) {
                    return ErrorView(
                      snapshot.error!,
                      onRetry: () => setState(() {
                        _listing = _load();
                      }),
                    );
                  }
                  final listing = snapshot.data;
                  if (listing == null) {
                    return const Center(child: CircularProgressIndicator());
                  }
                  if (listing.items.isEmpty) {
                    return Center(child: Text(s.noResults));
                  }
                  return Column(
                    children: [
                      Expanded(
                        child: ListView(
                          children: [
                            for (final record in listing.items)
                              _tile(forge, record),
                          ],
                        ),
                      ),
                      Pager(
                        page: listing.page,
                        pages: listing.pages,
                        first: (listing.page - 1) * listing.perPage + 1,
                        last:
                            (listing.page - 1) * listing.perPage +
                            listing.items.length,
                        total: listing.total,
                        onPage: (page) => setState(() {
                          _query = _query.copyWith(page: page);
                          _listing = _load();
                        }),
                      ),
                    ],
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(s.cancel),
        ),
        if (widget.multiple)
          FilledButton(
            onPressed: () => Navigator.pop(context, _selected.toList()),
            child: Text(s.ok),
          ),
      ],
    );
  }

  Widget _tile(Forge forge, Json record) {
    final id = record['id'] as int;
    final title = Text(forge.format.title(widget.target, record));
    if (!widget.multiple) {
      return ListTile(
        title: title,
        selected: _selected.contains(id),
        onTap: () => Navigator.pop(context, [id]),
      );
    }
    return CheckboxListTile(
      title: title,
      value: _selected.contains(id),
      onChanged: (checked) => setState(
        () => checked == true ? _selected.add(id) : _selected.remove(id),
      ),
    );
  }
}
