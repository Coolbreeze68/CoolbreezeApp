import 'package:flutter/material.dart';
import 'package:forge_flutter/forge_flutter.dart';

/// Couleur dont dérivent les thèmes clair et sombre (Material 3) ; les
/// dégradés de l'en-tête et de l'accueil sont dans `ForgeColors`.
const seedColor = ForgeColors.indigo;

final lightTheme = forgeTheme(seed: seedColor);

final darkTheme = forgeTheme(seed: seedColor, brightness: Brightness.dark);
