import 'package:flutter/material.dart';

/// Couleur dont dérivent les thèmes clair et sombre (Material 3).
const seedColor = Color(0xFF3F51B5);

final lightTheme = ThemeData(colorSchemeSeed: seedColor);

final darkTheme = ThemeData(
  colorSchemeSeed: seedColor,
  brightness: Brightness.dark,
);
