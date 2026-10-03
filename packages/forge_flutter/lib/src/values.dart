/// Conversions entre le JSON de l'API et les types Dart, et affichage des valeurs.
library;

import 'package:decimal/decimal.dart';
import 'package:intl/intl.dart';

import 'api/file.dart';
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
  late final _percent = NumberFormat.percentPattern(locale)
    ..maximumFractionDigits = 2;
  final _currencies = <String, NumberFormat>{};

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

  /// Proportion en pourcentage : `0.255` → `25,5 %`.
  String percent(Decimal value) => _percent.format(value.toDouble());

  /// Montant dans la devise `currency` : `1 500,00 €`.
  String money(Decimal value, String currency) => _currencies
      .putIfAbsent(
        currency,
        () => NumberFormat.simpleCurrency(locale: locale, name: currency),
      )
      .format(value.toDouble());

  /// Taille de fichier : `820 o`, `12 Ko`, `3,4 Mo`.
  String fileSize(int bytes) {
    final units = strings.sizeUnits;
    var size = bytes.toDouble();
    var unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
      size /= 1024;
      unit++;
    }
    final shown = unit == 0 || size >= 10
        ? _integer.format(size.round())
        : _decimal.format((size * 10).round() / 10);
    return '$shown ${units[unit]}';
  }

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
      ColumnType.string ||
      ColumnType.text ||
      ColumnType.color ||
      ColumnType.email ||
      ColumnType.url ||
      ColumnType.phone ||
      ColumnType.markdown => '$json',
      ColumnType.integer => json is num ? number(json) : '$json',
      ColumnType.rating => '$json/${column.max}',
      ColumnType.percent => switch (jsonToDecimal(json)) {
        final d? => percent(d),
        null => '$json',
      },
      ColumnType.money => switch (jsonToDecimal(json)) {
        final d? => money(d, column.currency ?? 'EUR'),
        null => '$json',
      },
      ColumnType.file ||
      ColumnType.image => ForgeFile.fromJson(json)?.name ?? '',
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

/// Pourcentage saisi (`12,5` pour 12,5 %), en proportion au format JSON (`0.125`).
String? parsePercentInput(String input) {
  final percent = Decimal.tryParse(parseDecimalInput(input) ?? '');
  return percent == null
      ? null
      : (percent / Decimal.fromInt(100))
            .toDecimal(scaleOnInfinitePrecision: 10)
            .toString();
}

/// Saisie plausible pour une colonne `email`, `url` ou `phone` (le serveur
/// fait foi ; ce contrôle évite un aller-retour pour une faute évidente).
bool isValidInput(ColumnType type, String text) => switch (type) {
  ColumnType.email => RegExp(r'^[^@\s]+@[^@\s]+\.[^@\s]+$').hasMatch(text),
  ColumnType.url => RegExp(r'^https?://[^\s/?#]+\S*$').hasMatch(text),
  ColumnType.phone =>
    RegExp(r'^\+?[\d\s().-]+$').hasMatch(text) &&
        RegExp(r'\d').allMatches(text).length >= 6,
  _ => true,
};

/// Entier saisi par l'utilisateur, espaces de groupement tolérés.
int? parseIntegerInput(String input) =>
    int.tryParse(input.replaceAll(RegExp(r'[\s  ]'), ''));
