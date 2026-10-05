# 6. Contribuer

Recettes pour les modifications les plus courantes, et la façon de vérifier
qu'on n'a rien cassé.

## Règles de travail

- Identifiants en anglais ; commentaires, documentation et messages d'erreur en
  français.
- Pas de code mort, pas de `TODO` vide, pas de fonctionnalité simulée.
- La logique va dans les bibliothèques (`forge-runtime`, `@forge/web`,
  `forge_flutter`), pas dans le code généré.
- Une fonction de l'interface existe **dans les deux** bibliothèques, avec le
  même comportement.
- Un choix structurant qui n'est pas couvert par l'existant se discute avant
  d'être codé ; une fois pris, il est noté dans [`CLAUDE.md`](../CLAUDE.md).

## Où faire une modification ?

| Je veux… | Fichiers |
|---|---|
| une nouvelle clé dans `forge.json` | `forge-schema/src/spec.rs`, puis `validate.rs` ; régénérer `forge.schema.json` |
| une règle de validation du schéma | `forge-schema/src/validate.rs` + un cas dans `forge-schema/tests/validation.rs` |
| une fonction de formule intégrée | `forge-formula/src/functions.rs` (signature + implémentation) |
| changer un comportement de l'API | `forge-runtime/src/resource.rs` ou le module concerné (jamais dans `rest`, `graphql` ou `csv_io` seul) |
| changer un fichier généré | `templates/…/*.j2` ou le générateur `forge-codegen/src/{backend,web,flutter}.rs` ; régénérer le CRM |
| changer un écran | `packages/forge_web/src/ui/` **et** `packages/forge_flutter/lib/src/ui/` |
| ajouter un texte d'interface | `i18n.ts` et `l10n/strings.dart`, en français et en anglais |

## Recette : ajouter un type de colonne

C'est la modification qui traverse le plus de couches ; prenons un modèle de
champ fictif `siret` (texte de 14 chiffres).

1. **Schéma** (`forge-schema`)
   - `spec.rs` : ajouter `Siret` à `ColumnType`, et dans `base()` le faire
     reposer sur `String`.
   - `value.rs` : sa validation (fonction `check`).
   - `validate.rs` : ses options éventuelles et leurs vérifications ; un cas de
     test dans `tests/validation.rs`.
   - `cargo run -p forge-cli -- schema > forge.schema.json`.
2. **Runtime** : en principe rien, puisque tout passe par `base()`. Vérifier
   les `match` sur `ColumnType` signalés par le compilateur (OpenAPI, GraphQL).
3. **Générateur** : le type TypeScript / Dart du modèle (`web.rs`,
   `flutter.rs`), et les options transmises aux interfaces
   (`frontend::field_options`).
4. **Interfaces** : ajouter le type à `schema.ts` / `schema.dart`, son
   affichage (`ValueFormat`) et son champ (`ui/models` / `ui/field_models`),
   avec un test de chaque côté.
5. **Exemple et docs** : l'utiliser dans le CRM si c'est pertinent,
   régénérer, mettre à jour le README (tableau des types) et `CLAUDE.md`.

## Recette : ajouter une règle de validation du schéma

Exemple réel, tiré de `Validator::check_model_options` :

```rust
// forge-schema/src/validate.rs
if let Some(max) = column.max
    && !(1..=10).contains(&max)
{
    self.error(format!("{path}.max"), "la note maximale va de 1 à 10");
}
```

Puis dans `tests/validation.rs`, un schéma de base modifié **d'une seule
erreur**, et le message attendu avec son chemin. Les erreurs sont collectées :
ne jamais interrompre la validation.

## Recette : modifier un template

1. Modifier `templates/<cible>/<fichier>.j2` (embarqué automatiquement s'il
   est déjà listé dans `forge-codegen/src/render.rs` ; sinon l'y ajouter).
2. `cargo run -p forge-cli -- generate --dir examples/crm`.
3. Relire le diff de `examples/crm` : c'est exactement ce que verront les
   projets des utilisateurs.
4. Un fichier `Once` (code utilisateur) ne sera **pas** mis à jour dans les
   projets existants : à garder en tête avant d'y mettre une correction.

## Vérifier

| Partie | Commandes |
|---|---|
| Rust | `cargo fmt --check` · `cargo clippy --all-targets --all-features -- -D warnings` · `cargo test --all-features` |
| Bibliothèque web | `cd packages/forge_web && npm ci && npm run lint && npm run typecheck && npm test` |
| Bibliothèque Flutter | `cd packages/forge_flutter && dart format --set-exit-if-changed lib test && flutter analyze && flutter test` |
| CRM web | `cd examples/crm/web && npm ci && npm run typecheck && npm test && npm run build` |
| CRM Flutter | `cd examples/crm/app && dart format --set-exit-if-changed lib test && flutter analyze && flutter test` |
| Autres bases | `TEST_DATABASE_URL=postgres://… cargo test -p mini_crm` (idem MySQL) |
| Scénario complet | `./scripts/e2e-evolution.sh` : schéma qui évolue en plusieurs étapes, migrations appliquées sur une vraie base |

`cargo test` inclut les tests du CRM (CRUD de toutes les tables, règles, CSV,
GraphQL, formules, hooks) et les deux tests de cohérence
(`forge.schema.json` et `examples/crm` à jour). La CI exécute le tout sur
PostgreSQL 16 et MySQL 8.4 et construit l'image Docker du CRM.

### Les tests générés

Chaque projet reçoit des tests qui suivent son schéma :

- backend : `tests/generated_crud.rs` appelle `check_resources` (création,
  lecture, modification, suppression de chaque table en REST, aller-retour CSV,
  GraphQL) et `check_rules` (droits de chaque rôle) ;
- interfaces : `app.test.tsx` (web) et `generated_test.dart` (Flutter) ouvrent
  chaque liste et chaque formulaire de création avec `FakeApi` ; côté Flutter,
  le test vérifie aussi la lecture et l'écriture JSON de chaque modèle typé.

Une évolution du runtime qui casse un projet est donc détectée par les tests
du CRM.
