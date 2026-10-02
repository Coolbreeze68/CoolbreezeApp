// NE PAS MODIFIER : code généré par forge depuis `forge.json`.
// Ce dossier est réécrit à chaque `forge generate` ; votre code va dans
// `lib/custom/`.

import 'package:flutter/widgets.dart';
import 'package:forge_flutter/forge_flutter.dart';

import '../custom/customization.dart';
import 'schema.dart';

export 'schema.dart';

/// Application complète ; `client` remplace le client par défaut (tests).
Widget buildApp({ForgeClient? client}) => ForgeApp(
  schema: schema,
  client: client ?? ForgeClient(baseUrl: defaultApiUrl()),
  customization: customization,
);
