import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:intl/date_symbol_data_local.dart';

import 'support.dart';

void main() {
  setUpAll(() => initializeDateFormatting());

  ValueFormat format(String locale) => ValueFormat(
    locale: locale,
    fallbackLocale: 'fr',
    strings: ForgeStrings.of(locale),
    enumLabels: const {
      'opportunite.etape': {
        'gagne': Label({'fr': 'Gagnée', 'en': 'Won'}),
      },
    },
  );

  final opportunites = schema.table('opportunite')!;
  ColumnSchema column(String name) => opportunites.column(name)!;

  test('conversions JSON', () {
    expect(jsonToDate('2026-10-02'), DateTime(2026, 10, 2));
    expect(dateToJson(DateTime(2026, 1, 5, 13)), '2026-01-05');
    final instant = jsonToDateTime('2026-10-02T08:30:00Z')!;
    expect(dateTimeToJson(instant), '2026-10-02T08:30:00.000Z');
    expect(jsonToDuration(5400), const Duration(minutes: 90));
    expect(jsonToDecimal('1500.25').toString(), '1500.25');
    expect(jsonToIds([1, 'x', 3]), [1, 3]);
    expect(jsonToDate(null), isNull);
  });

  test('mise en forme selon la langue', () {
    final fr = format('fr'), en = format('en');
    expect(fr.format(opportunites, column('montant'), '1234.5'), '1 234,5');
    expect(en.format(opportunites, column('montant'), '1234.5'), '1,234.5');
    expect(
      fr.format(opportunites, column('date_cloture'), '2026-10-02'),
      '02/10/2026',
    );
    expect(
      en.format(opportunites, column('date_cloture'), '2026-10-02'),
      '10/2/2026',
    );
    expect(fr.format(opportunites, column('etape'), 'gagne'), 'Gagnée');
    expect(en.format(opportunites, column('etape'), 'gagne'), 'Won');
    expect(fr.format(opportunites, column('etape'), 'prospect'), 'Prospect');
    expect(fr.format(opportunites, column('montant'), null), '');
    expect(fr.duration(const Duration(minutes: 90)), '1 h 30');
    expect(fr.duration(const Duration(minutes: 45)), '45 min');
    expect(fr.duration(const Duration(hours: 2)), '2 h');
    expect(fr.columnLabel(column('date_cloture')), 'Date cloture');
    expect(en.tableLabel(opportunites), 'Opportunity');
  });

  test('intitulé : colonnes title_field, sinon identifiant', () {
    final fr = format('fr');
    expect(fr.title(opportunites, {'id': 3, 'titre': 'Contrat'}), 'Contrat');
    expect(fr.title(opportunites, {'id': 3, 'titre': null}), '#3');
  });

  test('saisies numériques', () {
    expect(parseDecimalInput('1 234,50'), '1234.5');
    expect(parseDecimalInput('12.5'), '12.5');
    expect(parseDecimalInput('douze'), isNull);
    expect(parseIntegerInput('1 000'), 1000);
    expect(parseIntegerInput('1,5'), isNull);
  });
}
