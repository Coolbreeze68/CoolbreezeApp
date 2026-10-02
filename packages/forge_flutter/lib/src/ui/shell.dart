import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import 'widgets.dart';

/// Cadre des pages connectées : menu fixe sur grand écran ; sur petit écran,
/// chaque page ([ForgeScaffold]) ouvre le menu en tiroir.
class AppShell extends StatelessWidget {
  const AppShell({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (!isWide(context)) return child;
    return Row(
      children: [
        const SizedBox(width: 264, child: Material(child: NavMenu())),
        const VerticalDivider(width: 1),
        Expanded(child: child),
      ],
    );
  }
}

/// Page standard : barre de titre, et menu en tiroir sur petit écran (pages
/// de premier niveau).
class ForgeScaffold extends StatelessWidget {
  const ForgeScaffold({
    super.key,
    required this.title,
    required this.body,
    this.actions = const [],
    this.floatingActionButton,
    this.bottom,
  });

  final Widget title;
  final Widget body;
  final List<Widget> actions;
  final Widget? floatingActionButton;
  final PreferredSizeWidget? bottom;

  @override
  Widget build(BuildContext context) {
    // Une page empilée (fiche, formulaire) affiche le retour plutôt que le menu,
    // qui reste accessible par glissement.
    final stacked = ModalRoute.of(context)?.canPop ?? false;
    return Scaffold(
      appBar: AppBar(
        leading: stacked ? const BackButton() : null,
        title: title,
        actions: actions,
        bottom: bottom,
      ),
      drawer: isWide(context) ? null : const Drawer(child: NavMenu()),
      body: body,
      floatingActionButton: floatingActionButton,
    );
  }
}

class NavMenu extends StatelessWidget {
  const NavMenu({super.key});

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final location = GoRouterState.of(context).uri.path;
    final user = forge.user;

    Widget item(String path, IconData icon, String label) {
      final selected = location == path || location.startsWith('$path/');
      return ListTile(
        leading: Icon(icon),
        title: Text(label, overflow: TextOverflow.ellipsis),
        selected: selected,
        onTap: () {
          Scaffold.maybeOf(context)?.closeDrawer();
          context.go(path);
        },
      );
    }

    return SafeArea(
      child: ListView(
        children: [
          ListTile(
            title: Text(
              forge.schema.name,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            subtitle: user == null ? null : Text(user.name),
          ),
          const Divider(),
          for (final table in forge.schema.tables)
            if (forge.can(table, Operation.read))
              item(
                Paths.table(table.name),
                forge.customization.tableIcons[table.name] ??
                    Icons.table_rows_outlined,
                forge.format.tableLabel(table),
              ),
          for (final page in forge.customization.pages)
            if (page.roles?.any(forge.roles.contains) ?? true)
              item(
                Paths.page(page.path),
                page.icon,
                forge.format.label(page.label, page.path),
              ),
          const Divider(),
          if (forge.isAdmin && forge.schema.parameters.isNotEmpty)
            item(Paths.parameters, Icons.tune, s.parameters),
          if (forge.isAdmin) item(Paths.users, Icons.group_outlined, s.users),
          item(Paths.account, Icons.account_circle_outlined, s.account),
          ListTile(
            leading: const Icon(Icons.logout),
            title: Text(s.signOut),
            onTap: forge.client.signOut,
          ),
        ],
      ),
    );
  }
}
