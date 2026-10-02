import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';

import 'support.dart';

void main() {
  final opportunites = schema.table('opportunite')!;
  final entreprises = schema.table('entreprise')!;

  test('droits par rôle, admin a tout', () {
    expect(opportunites.allows(['admin'], Operation.delete), isTrue);
    expect(opportunites.allows(['commercial'], Operation.create), isTrue);
    expect(opportunites.allows(['commercial'], Operation.update), isTrue);
    expect(opportunites.allows(['lecteur'], Operation.create), isFalse);
    expect(entreprises.allows(['commercial'], Operation.create), isFalse);
    expect(entreprises.allows([], Operation.read), isFalse);
  });

  test('colonnes affichées, saisies et recherche', () {
    expect(
      opportunites.visibleColumns.map((c) => c.name),
      isNot(contains('notes')),
    );
    expect(entreprises.editableColumns.map((c) => c.name), ['nom', 'secteur']);
    expect(opportunites.searchable, isTrue);
  });

  test('listes liées : références vers la table', () {
    final related = schema.relatedLists(entreprises);
    expect(related.single.table.name, 'opportunite');
    expect(related.single.column.name, 'entreprise');
    expect(schema.relatedLists(opportunites), isEmpty);
  });

  test('paramètres de liste', () {
    final query =
        const ListQuery(
          page: 2,
          sort: [Sort('montant', descending: true), Sort('titre')],
          search: ' acme ',
        ).where([
          const Filter('montant', FilterOp.gte, '1000'),
          Filter.oneOf('etape', ['prospect', 'gagne']),
        ]);
    expect(query.toParameters(), {
      'page': '1',
      'per_page': '25',
      'sort': '-montant,titre',
      'q': 'acme',
      'montant[gte]': '1000',
      'etape[in]': 'prospect,gagne',
    });
    expect(query.toParameters(paging: false).containsKey('page'), isFalse);
    // Un filtre de même clé remplace le précédent.
    final replaced = query.where([const Filter('montant', FilterOp.gte, '5')]);
    expect(replaced.filters, hasLength(2));
    expect(replaced.toParameters()['montant[gte]'], '5');
    expect(query, query.copyWith());
    expect(query == replaced, isFalse);
  });
}
