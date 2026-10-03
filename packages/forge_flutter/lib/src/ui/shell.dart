import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../forge.dart';
import '../router.dart';
import '../schema.dart';
import '../theme.dart';
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

/// Page standard : barre de titre en dégradé, et menu en tiroir sur petit
/// écran (pages de premier niveau).
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
        foregroundColor: Colors.white,
        flexibleSpace: const SizedBox.expand(
          child: DecoratedBox(
            decoration: BoxDecoration(gradient: ForgeColors.gradient),
          ),
        ),
      ),
      drawer: isWide(context) ? null : const Drawer(child: NavMenu()),
      body: body,
      floatingActionButton: floatingActionButton,
    );
  }
}

/// Icône d'entrée de menu, sur fond teinté.
class TintedIcon extends StatelessWidget {
  const TintedIcon(this.icon, {super.key, this.color, this.size = 32});

  final IconData icon;
  final Color? color;
  final double size;

  @override
  Widget build(BuildContext context) {
    final tint = color ?? Theme.of(context).colorScheme.primary;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: tint.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Icon(icon, size: size * 0.55, color: tint),
    );
  }
}

class NavMenu extends StatelessWidget {
  const NavMenu({super.key});

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final theme = Theme.of(context);
    final location = GoRouterState.of(context).uri.path;
    final user = forge.user;

    Widget item(String path, Widget icon, String label, {bool exact = false}) {
      final selected =
          location == path || (!exact && location.startsWith('$path/'));
      return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
        child: ListTile(
          dense: true,
          leading: icon,
          title: Text(label, overflow: TextOverflow.ellipsis),
          selected: selected,
          selectedTileColor: theme.colorScheme.primaryContainer.withValues(
            alpha: 0.6,
          ),
          onTap: () {
            Scaffold.maybeOf(context)?.closeDrawer();
            context.go(path);
          },
        ),
      );
    }

    Widget section(String text) => Padding(
      padding: const EdgeInsets.fromLTRB(24, 16, 16, 6),
      child: Text(
        text.toUpperCase(),
        style: theme.textTheme.labelSmall?.copyWith(
          fontWeight: FontWeight.w700,
          color: theme.colorScheme.onSurfaceVariant,
          letterSpacing: 0.8,
        ),
      ),
    );

    return SafeArea(
      child: ListView(
        padding: EdgeInsets.zero,
        children: [
          Container(
            decoration: const BoxDecoration(gradient: ForgeColors.gradient),
            padding: const EdgeInsets.fromLTRB(16, 20, 16, 20),
            child: Row(
              children: [
                Container(
                  width: 40,
                  height: 40,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    forge.schema.name.substring(0, 1).toUpperCase(),
                    style: const TextStyle(
                      fontWeight: FontWeight.w800,
                      fontSize: 20,
                      color: ForgeColors.indigoDark,
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        forge.schema.name,
                        style: theme.textTheme.titleMedium?.copyWith(
                          color: Colors.white,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      if (user != null)
                        Text(
                          user.name,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: Colors.white70,
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 8),
          item(
            Paths.home,
            const TintedIcon(Icons.dashboard_outlined),
            s.dashboard,
            exact: true,
          ),
          section(s.records),
          for (final table in forge.schema.tables)
            if (forge.can(table, Operation.read))
              item(
                Paths.table(table.name),
                TintedIcon(
                  forge.customization.tableIcons[table.name] ??
                      Icons.table_rows_outlined,
                ),
                forge.format.tableLabel(table),
              ),
          for (final page in forge.customization.pages)
            if (page.roles?.any(forge.roles.contains) ?? true)
              item(
                Paths.page(page.path),
                TintedIcon(page.icon, color: const Color(0xFFBE4BDB)),
                forge.format.label(page.label, page.path),
              ),
          const Divider(indent: 16, endIndent: 16, height: 24),
          if (forge.isAdmin && forge.schema.parameters.isNotEmpty)
            item(Paths.parameters, const Icon(Icons.tune), s.parameters),
          if (forge.isAdmin)
            item(Paths.users, const Icon(Icons.group_outlined), s.users),
          item(
            Paths.account,
            const Icon(Icons.account_circle_outlined),
            s.account,
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child: ListTile(
              dense: true,
              leading: Icon(Icons.logout, color: theme.colorScheme.error),
              title: Text(s.signOut),
              onTap: forge.client.signOut,
            ),
          ),
        ],
      ),
    );
  }
}
