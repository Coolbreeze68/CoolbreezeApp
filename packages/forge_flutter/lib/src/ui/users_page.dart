import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../api/client.dart';
import '../forge.dart';
import '../router.dart';
import 'shell.dart';
import 'widgets.dart';

/// Comptes utilisateurs (administrateurs).
class UsersPage extends StatefulWidget {
  const UsersPage({super.key});

  @override
  State<UsersPage> createState() => _UsersPageState();
}

class _UsersPageState extends State<UsersPage> {
  static const _perPage = 25;
  var _page = 1;
  var _search = '';
  Future<Json>? _listing;
  Timer? _debounce;
  DataChanges? _changes;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final changes = Forge.of(context).changes;
    if (changes != _changes) {
      _changes?.removeListener(_reload);
      _changes = changes..addListener(_reload);
    }
    _listing ??= _load();
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _changes?.removeListener(_reload);
    super.dispose();
  }

  Future<Json> _load() async =>
      await Forge.of(context).client.get(
            '/api/users',
            query: {
              'page': '$_page',
              'per_page': '$_perPage',
              if (_search.isNotEmpty) 'q': _search,
            },
          )
          as Json;

  void _reload() => setState(() {
    _listing = _load();
  });

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    return ForgeScaffold(
      title: Text(s.users),
      floatingActionButton: FloatingActionButton(
        tooltip: s.create,
        onPressed: () => context.push('${Paths.users}/new'),
        child: const Icon(Icons.person_add),
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: TextField(
              decoration: InputDecoration(
                prefixIcon: const Icon(Icons.search),
                hintText: s.search,
                isDense: true,
              ),
              onChanged: (text) {
                _debounce?.cancel();
                _debounce = Timer(const Duration(milliseconds: 300), () {
                  _search = text.trim();
                  _page = 1;
                  _reload();
                });
              },
            ),
          ),
          Expanded(
            child: FutureBuilder(
              future: _listing,
              builder: (context, snapshot) {
                if (snapshot.hasError) {
                  return ErrorView(snapshot.error!, onRetry: _reload);
                }
                final listing = snapshot.data;
                if (listing == null) {
                  return const Center(child: CircularProgressIndicator());
                }
                final users = [
                  for (final user in listing['data'] as List)
                    ForgeUser.fromJson(user as Json),
                ];
                final total = listing['total'] as int;
                final first = (_page - 1) * _perPage + 1;
                return Column(
                  children: [
                    Expanded(
                      child: ListView(
                        children: [
                          for (final user in users)
                            ListTile(
                              leading: Icon(
                                user.active
                                    ? Icons.person_outline
                                    : Icons.person_off_outlined,
                              ),
                              title: Text(user.name),
                              subtitle: Text(
                                [user.email, ...user.roles].join(' · '),
                              ),
                              onTap: () => context.push(Paths.user(user.id)),
                            ),
                        ],
                      ),
                    ),
                    Pager(
                      page: _page,
                      pages: total == 0
                          ? 1
                          : (total + _perPage - 1) ~/ _perPage,
                      first: users.isEmpty ? 0 : first,
                      last: first + users.length - 1,
                      total: total,
                      onPage: (page) {
                        _page = page;
                        _reload();
                      },
                    ),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

/// Création (`id` absent) ou modification d'un compte.
class UserFormPage extends StatefulWidget {
  const UserFormPage({super.key, this.id});

  final int? id;

  @override
  State<UserFormPage> createState() => _UserFormPageState();
}

class _UserFormPageState extends State<UserFormPage> {
  final _form = GlobalKey<FormState>();
  final _email = TextEditingController();
  final _name = TextEditingController();
  final _password = TextEditingController();
  Set<String> _roles = {};
  bool _active = true;
  ForgeUser? _user;
  Object? _loadError;
  Map<String, List<String>> _errors = const {};
  bool _saving = false;

  bool get _creating => widget.id == null;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_creating && _user == null && _loadError == null) _load();
  }

  @override
  void dispose() {
    _email.dispose();
    _name.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final json =
          await Forge.of(context).client.get('/api/users/${widget.id}') as Json;
      final user = ForgeUser.fromJson(json);
      if (!mounted) return;
      setState(() {
        _user = user;
        _email.text = user.email;
        _name.text = user.displayName ?? '';
        _roles = {...user.roles};
        _active = user.active;
      });
    } on ApiException catch (error) {
      if (mounted) setState(() => _loadError = error);
    }
  }

  Future<void> _save() async {
    if (!_form.currentState!.validate()) return;
    final forge = Forge.of(context);
    final body = {
      'email': _email.text.trim(),
      'display_name': _name.text.trim().isEmpty ? null : _name.text.trim(),
      'roles': _roles.toList(),
      'active': _active,
      if (_password.text.isNotEmpty) 'password': _password.text,
    };
    setState(() => _saving = true);
    try {
      if (_creating) {
        await forge.client.post('/api/users', body);
      } else {
        await forge.client.patch('/api/users/${widget.id}', body);
      }
      // Son propre compte : nom et rôles affichés à jour.
      if (widget.id == forge.user?.id) await forge.client.reloadUser();
      forge.changes.notify();
      if (mounted) context.go(Paths.users);
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() => _errors = error.fields);
      if (error.fields.isEmpty) showError(context, error);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _delete() async {
    final forge = Forge.of(context);
    final s = forge.strings;
    if (!await confirm(
      context,
      s.confirmDelete(_user!.name),
      action: s.delete,
    )) {
      return;
    }
    try {
      await forge.client.delete('/api/users/${widget.id}');
      forge.changes.notify();
      if (mounted) context.go(Paths.users);
    } on ApiException catch (error) {
      if (mounted) showError(context, error);
    }
  }

  String? _error(String field) => _errors[field]?.join('\n');

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final Widget body;
    if (_loadError case final error?) {
      body = ErrorView(error, onRetry: _load);
    } else if (!_creating && _user == null) {
      body = const Center(child: CircularProgressIndicator());
    } else {
      body = SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Constrained(
          child: Form(
            key: _form,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextFormField(
                  controller: _email,
                  decoration: InputDecoration(
                    labelText: '${s.email} *',
                    errorText: _error('email'),
                  ),
                  keyboardType: TextInputType.emailAddress,
                  validator: (v) => v!.trim().isEmpty ? s.required : null,
                ),
                const SizedBox(height: 16),
                TextFormField(
                  controller: _name,
                  decoration: InputDecoration(
                    labelText: s.displayName,
                    errorText: _error('display_name'),
                  ),
                ),
                const SizedBox(height: 16),
                TextFormField(
                  controller: _password,
                  obscureText: true,
                  decoration: InputDecoration(
                    labelText: _creating ? '${s.password} *' : s.newPassword,
                    helperText: _creating ? null : s.keepPasswordHint,
                    errorText: _error('password'),
                  ),
                  validator: (v) => _creating && v!.isEmpty ? s.required : null,
                ),
                const SizedBox(height: 16),
                InputDecorator(
                  decoration: InputDecoration(
                    labelText: s.roles,
                    errorText: _error('roles'),
                  ),
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      for (final role in forge.schema.roles)
                        FilterChip(
                          label: Text(role),
                          selected: _roles.contains(role),
                          onSelected: (on) => setState(
                            () => on ? _roles.add(role) : _roles.remove(role),
                          ),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 8),
                SwitchListTile(
                  title: Text(s.active),
                  value: _active,
                  onChanged: (v) => setState(() => _active = v),
                ),
              ],
            ),
          ),
        ),
      );
    }
    return ForgeScaffold(
      title: Text(_creating ? s.newUser : (_user?.name ?? s.users)),
      actions: [
        if (!_creating && _user != null && _user!.id != forge.user?.id)
          IconButton(
            tooltip: s.delete,
            icon: const Icon(Icons.delete_outline),
            onPressed: _delete,
          ),
        Padding(
          padding: const EdgeInsets.only(right: 8),
          child: FilledButton.icon(
            onPressed: _saving ? null : _save,
            icon: const Icon(Icons.check),
            label: Text(s.save),
          ),
        ),
      ],
      body: body,
    );
  }
}
