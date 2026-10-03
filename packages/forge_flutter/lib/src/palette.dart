import 'package:flutter/material.dart';

/// Palette catégorielle commune avec l'application web : chaque valeur
/// d'énumération garde sa couleur partout, dans l'ordre de déclaration.
/// L'ordre est validé pour les daltonismes (écart entre voisines) ; au-delà de
/// huit valeurs, la couleur neutre est utilisée.
abstract final class Palette {
  static const light = [
    Color(0xFF2A78D6),
    Color(0xFFEB6834),
    Color(0xFF1BAF7A),
    Color(0xFFEDA100),
    Color(0xFFE87BA4),
    Color(0xFF008300),
    Color(0xFF4A3AA7),
    Color(0xFFE34948),
  ];

  static const dark = [
    Color(0xFF3987E5),
    Color(0xFFD95926),
    Color(0xFF199E70),
    Color(0xFFC98500),
    Color(0xFFD55181),
    Color(0xFF008300),
    Color(0xFF9085E9),
    Color(0xFFE66767),
  ];

  static const neutral = Color(0xFF898781);
}

/// Couleur de la valeur d'énumération `value` parmi `values`.
Color enumColor(List<String> values, String value, Brightness brightness) {
  final palette = brightness == Brightness.dark ? Palette.dark : Palette.light;
  final index = values.indexOf(value);
  return index >= 0 && index < palette.length
      ? palette[index]
      : Palette.neutral;
}
