import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import 'api/client.dart';
import 'forge.dart';
import 'schema.dart';
import 'ui/account_page.dart';
import 'ui/detail_page.dart';
import 'ui/form_page.dart';
import 'ui/home_page.dart';
import 'ui/login_page.dart';
import 'ui/parameters_page.dart';
import 'ui/shell.dart';
import 'ui/table_page.dart';
import 'ui/users_page.dart';
import 'ui/widgets.dart';

/// Chemins de l'application.
abstract final class Paths {
  static const home = '/';
  static const login = '/login';
  static const account = '/account';
  static const parameters = '/settings/parameters';
  static const users = '/settings/users';

  static String table(String table) => '/data/$table';
  static String record(String table, int id) => '/data/$table/$id';
  static String edit(String table, int id) => '/data/$table/$id/edit';

  /// Création, valeurs initiales passées en paramètres d'URL (format de l'API).
  static String create(
    String table, [
    Map<String, String> initial = const {},
  ]) => Uri(
    path: '/data/$table/new',
    queryParameters: initial.isEmpty ? null : initial,
  ).toString();

  /// Création qui revient avec l'identifiant créé au lieu d'ouvrir la fiche
  /// (depuis un champ de référence).
  static String pick(String table) => create(table, {pickParameter: '1'});

  /// Paramètre d'URL du mode « choix » ; une colonne ne peut pas le porter
  /// (un nom de colonne commence par une lettre).
  static const pickParameter = '_pick';

  static String user(int id) => '$users/$id';
  static String page(String path) => '/pages/$path';
}

GoRouter buildRouter(AppSchema schema, ForgeClient client) => GoRouter(
  refreshListenable: client,
  redirect: (context, state) {
    final atLogin = state.matchedLocation == Paths.login;
    if (!client.signedIn) {
      if (atLogin) return null;
      return Uri(
        path: Paths.login,
        queryParameters: state.matchedLocation == '/'
            ? null
            : {'from': state.uri.toString()},
      ).toString();
    }
    if (atLogin) return state.uri.queryParameters['from'] ?? Paths.home;
    return null;
  },
  errorBuilder: (context, state) => const Scaffold(body: NotFound()),
  routes: [
    GoRoute(path: Paths.login, builder: (_, _) => const LoginPage()),
    ShellRoute(
      builder: (context, state, child) => AppShell(child: child),
      routes: [
        GoRoute(path: Paths.home, builder: (_, _) => const HomePage()),
        GoRoute(
          path: '/data/:table',
          builder: (context, state) => _withTable(
            context,
            state,
            (table) => TablePage(
              key: ValueKey(state.uri.toString()),
              table: table,
              initialFilters: state.uri.queryParameters,
            ),
          ),
          routes: [
            GoRoute(
              path: 'new',
              builder: (context, state) => _withTable(context, state, (table) {
                final parameters = {...state.uri.queryParameters};
                final pick = parameters.remove(Paths.pickParameter) != null;
                return FormPage(
                  table: table,
                  initial: parameters,
                  returnsId: pick,
                );
              }),
            ),
            GoRoute(
              path: ':id',
              builder: (context, state) => _withRecord(
                context,
                state,
                (table, id) =>
                    DetailPage(key: ValueKey('$id'), table: table, id: id),
              ),
              routes: [
                GoRoute(
                  path: 'edit',
                  builder: (context, state) => _withRecord(
                    context,
                    state,
                    (table, id) => FormPage(table: table, id: id),
                  ),
                ),
              ],
            ),
          ],
        ),
        GoRoute(
          path: '/pages/:page',
          builder: (context, state) {
            final forge = Forge.of(context);
            for (final page in forge.customization.pages) {
              if (page.path == state.pathParameters['page'] &&
                  (page.roles?.any(forge.roles.contains) ?? true)) {
                return page.builder(context);
              }
            }
            return const NotFound();
          },
        ),
        GoRoute(path: Paths.account, builder: (_, _) => const AccountPage()),
        GoRoute(
          path: Paths.parameters,
          builder: (_, _) => const ParametersPage(),
        ),
        GoRoute(
          path: Paths.users,
          builder: (_, _) => const UsersPage(),
          routes: [
            GoRoute(path: 'new', builder: (_, _) => const UserFormPage()),
            GoRoute(
              path: ':id',
              builder: (context, state) =>
                  switch (int.tryParse(state.pathParameters['id']!)) {
                    final id? => UserFormPage(id: id),
                    null => const NotFound(),
                  },
            ),
          ],
        ),
      ],
    ),
  ],
);

Widget _withTable(
  BuildContext context,
  GoRouterState state,
  Widget Function(TableSchema table) builder,
) {
  final forge = Forge.of(context);
  final table = forge.schema.table(state.pathParameters['table']!);
  if (table == null || !forge.can(table, Operation.read)) {
    return const NotFound();
  }
  return builder(table);
}

Widget _withRecord(
  BuildContext context,
  GoRouterState state,
  Widget Function(TableSchema table, int id) builder,
) => _withTable(context, state, (table) {
  final id = int.tryParse(state.pathParameters['id']!);
  return id == null ? const NotFound() : builder(table, id);
});
