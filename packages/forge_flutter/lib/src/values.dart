/// Conversions entre le JSON de l'API et les types Dart, et affichage des valeurs.
library;

import 'package:decimal/decimal.dart';
import 'package:intl/intl.dart';

import 'l10n/strings.dart';
import 'schema.dart';

// ------------------------------------------------------------- JSON → Dart

/// `"2026-10-02"` → date locale à minuit.
DateTime? jsonToDate(Object? json) {
  if (json is! String) return null;
  final parsed = DateTime.tryParse(json);
  return parsed == null
      ? null
      : DateTime(parsed.year, parsed.month, parsed.day);
}

/// Date-heure RFC 3339 → heure locale.
DateTime? jsonToDateTime(Object? json) =>
    json is String ? DateTime.tryParse(json)?.toLocal() : null;

/// Secondes → durée.
Duration? jsonToDuration(Object? json) =>
    json is num ? Duration(seconds: json.toInt()) : null;

/// Les décimaux sont transmis en texte pour ne rien perdre en précision.
Decimal? jsonToDecimal(Object? json) => switch (json) {
  final String s => Decimal.tryParse(s),
  final int n => Decimal.fromInt(n),
  final double d => Decimal.tryParse('$d'),
  _ => null,
};

List<int> jsonToIds(Object? json) => [
  if (json is List)
    for (final id in json)
      if (id is int) id,
];

// ------------------------------------------------------------- Dart → JSON

String? dateToJson(DateTime? date) => date == null
    ? null
    : '${date.year.toString().padLeft(4, '0')}-'
          '${date.month.toString().padLeft(2, '0')}-'
          '${date.day.toString().padLeft(2, '0')}';

String? dateTimeToJson(DateTime? dateTime) =>
    dateTime?.toUtc().toIso8601String();

int? durationToJson(Duration? duration) => duration?.inSeconds;

String? decimalToJson(Decimal? decimal) => decimal?.toString();

// -------------------------------------------------------------- Affichage

/// Libellés personnalisés des valeurs d'énumération : `table.colonne` → valeur → libellé.
typedef EnumLabels = Map<String, Map<String, Label>>;

/// Mise en forme des valeurs dans la langue de l'interface.
class ValueFormat {
  ValueFormat({
    required this.locale,
    required this.fallbackLocale,
    required this.strings,
    this.enumLabels = const {},
  });

  final String locale;
  final String fallbackLocale;
  final ForgeStrings strings;
  final EnumLabels enumLabels;

  late final _integer = NumberFormat.decimalPattern(locale);
  late final _decimal = NumberFormat.decimalPattern(locale)
    ..maximumFractionDigits = 4;
  late final _date = DateFormat.yMd(locale);
  late final _dateTime = DateFormat.yMd(locale).add_Hm();
  late final _time = DateFormat.Hm(locale);

  String label(Label? label, String name) =>
      label?.resolve(locale, fallbackLocale) ?? humanize(name);

  String tableLabel(TableSchema table) => label(table.label, table.name);

  String columnLabel(ColumnSchema column) => label(column.label, column.name);

  String enumLabel(TableSchema table, ColumnSchema column, String value) =>
      enumLabels['${table.name}.${column.name}']?[value]?.resolve(
        locale,
        fallbackLocale,
      ) ??
      humanize(value);

  String number(num value) =>
      value is int ? _integer.format(value) : _decimal.format(value);

  String date(DateTime value) => _date.format(value);

  String dateTime(DateTime value) => _dateTime.format(value);

  String time(DateTime value) => _time.format(value);

  /// `1 h 30`, `45 min`, `2 h`.
  String duration(Duration value) {
    final negative = value.isNegative;
    final minutes = value.abs().inMinutes;
    final (h, m) = (minutes ~/ 60, minutes % 60);
    final text = switch ((h, m)) {
      (0, _) => '$m min',
      (_, 0) => '$h h',
      _ => '$h h ${m.toString().padLeft(2, '0')}',
    };
    return negative ? '-$text' : text;
  }

  /// Texte d'une valeur JSON de la colonne ; une référence s'affiche par son
  /// identifiant (l'interface la remplace par l'intitulé de l'enregistrement).
  String format(TableSchema table, ColumnSchema column, Object? json) {
    if (json == null) return '';
    return switch (column.type) {
      ColumnType.string || ColumnType.text => '$json',
      ColumnType.integer => json is num ? number(json) : '$json',
      ColumnType.decimal => switch (jsonToDecimal(json)) {
        final d? => _decimal.format(d.toDouble()),
        null => '$json',
      },
      ColumnType.boolean => json == true ? strings.yes : strings.no,
      ColumnType.date => switch (jsonToDate(json)) {
        final d? => date(d),
        null => '$json',
      },
      ColumnType.datetime => switch (jsonToDateTime(json)) {
        final d? => dateTime(d),
        null => '$json',
      },
      ColumnType.duration => switch (jsonToDuration(json)) {
        final d? => duration(d),
        null => '$json',
      },
      ColumnType.enumeration => enumLabel(table, column, '$json'),
      ColumnType.reference => '#$json',
      ColumnType.referenceList => jsonToIds(
        json,
      ).map((id) => '#$id').join(', '),
    };
  }

  /// Intitulé d'un enregistrement : ses colonnes `title_field`, sinon `#id`.
  String title(TableSchema table, Map<String, dynamic> record) {
    final parts = [
      for (final column in table.titleColumns)
        format(table, column, record[column.name]),
    ].where((part) => part.isNotEmpty);
    return parts.isEmpty ? '#${record['id']}' : parts.join(' ');
  }
}

/// `date_cloture` → `Date cloture`.
String humanize(String name) {
  final words = name.replaceAll('_', ' ').trim();
  return words.isEmpty ? name : words[0].toUpperCase() + words.substring(1);
}

/// Décimal saisi par l'utilisateur (`1 234,5` ou `1234.5`), au format JSON.
String? parseDecimalInput(String input) {
  final normalized = input
      .replaceAll(RegExp(r'[\s  ]'), '')
      .replaceAll(',', '.');
  return Decimal.tryParse(normalized)?.toString();
}

/// Entier saisi par l'utilisateur, espaces de groupement tolérés.
int? parseIntegerInput(String input) =>
    int.tryParse(input.replaceAll(RegExp(r'[\s  ]'), ''));
