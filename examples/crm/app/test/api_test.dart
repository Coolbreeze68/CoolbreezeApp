// Parcours complet contre un backend réel, avec les modèles typés.
// Lancé seulement si FORGE_E2E_URL est défini :
//   FORGE_E2E_URL=http://localhost:8080 FORGE_E2E_EMAIL=… FORGE_E2E_PASSWORD=… flutter test test/api_test.dart

import 'dart:convert';
import 'dart:io';

import 'package:decimal/decimal.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forge_flutter/forge_flutter.dart';
import 'package:mini_crm_app/generated/models.dart';

void main() {
  final url = Platform.environment['FORGE_E2E_URL'];

  test(
    'API du CRM : CRUD, formules, agrégats, CSV',
    () async {
      final client = ForgeClient(
        baseUrl: Uri.parse(url!),
        store: MemoryStore(),
      );
      await client.signIn(
        Platform.environment['FORGE_E2E_EMAIL'] ?? 'admin@crm.test',
        Platform.environment['FORGE_E2E_PASSWORD'] ?? 'motdepasse123',
      );
      final entreprises = Entreprise.api(client);
      final opportunites = Opportunite.api(client);
      final suffix = DateTime.now().microsecondsSinceEpoch;

      final acme = await entreprises.create({
        'nom': 'Acme $suffix',
        'secteur': 'industrie',
      });
      expect(acme.secteur, EntrepriseSecteur.industrie);

      final contrat = await opportunites.create({
        'titre': 'Contrat $suffix',
        'entreprise': acme.id,
        'montant': '1000',
      });
      // Défaut, formules persistées et lookup.
      expect(contrat.probabilite, 50);
      expect(contrat.etape, OpportuniteEtape.prospect);
      expect(contrat.montantPondere, Decimal.parse('500'));
      expect(contrat.secteur, EntrepriseSecteur.industrie);
      expect((await entreprises.read(acme.id)).pipeline, Decimal.parse('1000'));

      final byCompany = ListQuery(
        filters: [Filter.equals('entreprise', '${acme.id}')],
      );
      expect((await opportunites.list(byCompany)).items.single.id, contrat.id);

      final gagne = await opportunites.update(contrat.id, {'etape': 'gagne'});
      expect(gagne.etape, OpportuniteEtape.gagne);

      final stats = await opportunites.aggregate(
        ['montant'],
        groupBy: 'etape',
        query: byCompany,
      );
      expect(stats.total.count, 1);
      expect(stats.total.measures['montant']!.sum, 1000);
      expect(stats.groups.single.key, 'gagne');

      final csv = utf8.decode(await opportunites.export(query: byCompany));
      expect(csv, contains('Contrat $suffix'));
      final report = await opportunites.import(
        utf8.encode('id,titre\n${contrat.id},Contrat importé $suffix\n'),
      );
      expect(report.updated, 1);
      expect(
        (await opportunites.read(contrat.id)).titre,
        'Contrat importé $suffix',
      );

      await expectLater(
        opportunites.create({'titre': 'Sans montant', 'entreprise': acme.id}),
        throwsA(
          isA<ApiException>().having(
            (e) => e.fields.keys,
            'fields',
            contains('montant'),
          ),
        ),
      );

      await opportunites.delete(contrat.id);
      await entreprises.delete(acme.id);
      await expectLater(
        entreprises.read(acme.id),
        throwsA(isA<ApiException>().having((e) => e.status, 'status', 404)),
      );
      await client.signOut();
    },
    skip: url == null ? 'FORGE_E2E_URL non défini' : false,
  );
}
