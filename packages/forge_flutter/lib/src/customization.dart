import 'package:flutter/material.dart';

import 'api/client.dart';
import 'l10n/strings.dart';
import 'schema.dart';
import 'values.dart';

/// Champ de formulaire : valeur au format JSON de l'API et erreur à afficher.
class FieldState {
  const FieldState({
    required this.table,
    required this.column,
    required this.value,
    required this.onChanged,
    this.error,
  });

  final TableSchema table;
  final ColumnSchema column;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final String? error;
}

typedef FieldBuilder = Widget Function(BuildContext context, FieldState field);

/// Affichage d'une valeur (liste, détail) : enregistrement complet et valeur.
typedef CellBuilder =
    Widget Function(BuildContext context, Json record, Object? value);

/// Bloc ajouté à la page de détail d'un enregistrement.
typedef DetailSectionBuilder =
    Widget Function(BuildContext context, Json record);

/// Page ajoutée au menu, servie sous `/pages/<path>`.
class CustomPage {
  const CustomPage({
    required this.path,
    required this.label,
    required this.icon,
    required this.builder,
    this.roles,
  });

  final String path;
  final Label label;
  final IconData icon;
  final WidgetBuilder builder;

  /// Rôles qui voient la page (tous si `null`).
  final List<String>? roles;
}

/// Points d'extension de l'application générée, déclarés dans
/// `lib/custom/customization.dart` (jamais écrasé par `forge generate`).
/// Les clés `table.colonne` désignent une colonne.
class ForgeCustomization {
  const ForgeCustomization({
    this.theme,
    this.darkTheme,
    this.tableIcons = const {},
    this.enumLabels = const {},
    this.fields = const {},
    this.cells = const {},
    this.detailSections = const {},
    this.pages = const [],
    this.strings = const {},
  });

  final ThemeData? theme;
  final ThemeData? darkTheme;

  /// Icône d'une table dans le menu.
  final Map<String, IconData> tableIcons;

  /// Libellés des valeurs d'énumération (`table.colonne` → valeur → libellé).
  final EnumLabels enumLabels;

  /// Champs de formulaire remplaçant ceux par défaut (`table.colonne`).
  final Map<String, FieldBuilder> fields;

  /// Affichages de valeurs remplaçant ceux par défaut (`table.colonne`).
  final Map<String, CellBuilder> cells;

  /// Blocs supplémentaires de la page de détail, par table.
  final Map<String, List<DetailSectionBuilder>> detailSections;

  final List<CustomPage> pages;

  /// Textes de l'interface par langue, en plus ou à la place de `fr` et `en`.
  final Map<String, ForgeStrings> strings;
}
