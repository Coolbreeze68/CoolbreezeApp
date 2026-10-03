import 'package:decimal/decimal.dart';
import 'package:flutter/material.dart';
import 'package:forge_flutter/forge_flutter.dart';

import '../generated/models.dart';

/// Bloc de la fiche entreprise : opportunités gagnées et leur montant,
/// lues avec le modèle typé `Opportunite`.
class EntrepriseSummary extends StatelessWidget {
  const EntrepriseSummary({super.key, required this.id});

  final int id;

  Future<List<Opportunite>> _won(ForgeClient client) =>
      Opportunite.api(client).listAll(
        ListQuery(
          filters: [
            Filter.equals('entreprise', '$id'),
            const Filter.equals('etape', 'gagne'),
          ],
        ),
      );

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final french = forge.locale == 'fr';
    return FutureBuilder(
      future: _won(forge.client),
      builder: (context, snapshot) {
        final won = snapshot.data;
        if (won == null) return const SizedBox.shrink();
        final total = won.fold(Decimal.zero, (sum, o) => sum + o.montant);
        final amount = forge.format.number(total.toDouble());
        return StatTile(
          label: french ? 'Opportunités gagnées' : 'Won opportunities',
          value: '${won.length}',
          details: french ? 'Montant HT : $amount' : 'Amount: $amount',
          icon: Icons.emoji_events_outlined,
          gradient: const [Color(0xFFFAB005), Color(0xFFFD7E14)],
        );
      },
    );
  }
}
