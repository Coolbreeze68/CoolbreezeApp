import 'package:flutter/material.dart';

/// Identité visuelle « Material moderne », commune avec l'application web :
/// indigo, dégradé indigo → violet, coins arrondis.
abstract final class ForgeColors {
  static const indigo = Color(0xFF4C6EF5);
  static const indigoDark = Color(0xFF4263EB);
  static const violet = Color(0xFF7950F2);
  static const pink = Color(0xFFF06595);

  /// Dégradé des bandeaux (en-tête, accueil, connexion).
  static const gradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [indigoDark, violet],
  );

  /// Dégradé de la page de connexion et du bandeau d'accueil.
  static const vividGradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [indigoDark, violet, pink],
  );

  /// Dégradés des indicateurs de l'accueil, dans l'ordre des tables.
  static const tileGradients = [
    [Color(0xFF4C6EF5), Color(0xFF7950F2)],
    [Color(0xFF12B886), Color(0xFF15AABF)],
    [Color(0xFFFD7E14), Color(0xFFF06595)],
    [Color(0xFFBE4BDB), Color(0xFFF06595)],
    [Color(0xFF228BE6), Color(0xFF15AABF)],
    [Color(0xFF82C91E), Color(0xFF12B886)],
  ];
}

/// Thème de l'application ; `seed` change la couleur principale.
ThemeData forgeTheme({
  Brightness brightness = Brightness.light,
  Color seed = ForgeColors.indigo,
}) {
  final scheme = ColorScheme.fromSeed(seedColor: seed, brightness: brightness);
  final radius = BorderRadius.circular(10);
  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: brightness == Brightness.light
        ? const Color(0xFFF8F9FA)
        : const Color(0xFF1F1F1F),
    cardTheme: CardThemeData(
      elevation: 1,
      margin: EdgeInsets.zero,
      clipBehavior: Clip.antiAlias,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.6)),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      border: OutlineInputBorder(borderRadius: radius),
      isDense: true,
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        shape: RoundedRectangleBorder(borderRadius: radius),
      ),
    ),
    listTileTheme: ListTileThemeData(
      shape: RoundedRectangleBorder(borderRadius: radius),
    ),
    navigationDrawerTheme: const NavigationDrawerThemeData(elevation: 0),
  );
}
