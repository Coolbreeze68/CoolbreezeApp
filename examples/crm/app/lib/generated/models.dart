// NE PAS MODIFIER : code généré par forge depuis `forge.json`.
// Ce dossier est réécrit à chaque `forge generate` ; votre code va dans
// `lib/custom/`.

// Modèles typés des tables, pour le code personnalisé :
//   final listing = await Entreprise.api(client).list();

import 'package:decimal/decimal.dart';
import 'package:forge_flutter/forge_flutter.dart'
    show
        ForgeClient,
        ForgeFile,
        TableClient,
        dateTimeToJson,
        dateToJson,
        decimalToJson,
        durationToJson,
        jsonToDate,
        jsonToDateTime,
        jsonToDecimal,
        jsonToDuration,
        jsonToIds;

/// Entreprise (table `entreprise`).
class Entreprise {
  const Entreprise({
    required this.id,
    required this.nom,
    this.secteur,
    this.ville,
    this.siteWeb,
    this.logo,
    this.satisfaction,
    this.chiffreAffaires,
    this.presentation,
    this.pipeline,
    this.pipelinePondere,
    this.nbContacts,
    this.owner,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Entreprise.fromJson(Map<String, dynamic> json) => Entreprise(
    id: json['id'] as int,
    nom: json['nom'] as String,
    secteur: EntrepriseSecteur.fromJson(json['secteur']),
    ville: json['ville'] as String?,
    siteWeb: json['site_web'] as String?,
    logo: ForgeFile.fromJson(json['logo']),
    satisfaction: json['satisfaction'] as int?,
    chiffreAffaires: jsonToDecimal(json['chiffre_affaires']),
    presentation: json['presentation'] as String?,
    pipeline: jsonToDecimal(json['pipeline']),
    pipelinePondere: jsonToDecimal(json['pipeline_pondere']),
    nbContacts: json['nb_contacts'] as int?,
    owner: json['owner'] as int?,
    createdAt: jsonToDateTime(json['created_at'])!,
    updatedAt: jsonToDateTime(json['updated_at'])!,
  );

  static const table = 'entreprise';

  /// Opérations typées sur la table.
  static TableClient<Entreprise> api(ForgeClient client) => TableClient(
    client,
    table,
    Entreprise.fromJson,
  );

  final int id;
  final String nom;
  final EntrepriseSecteur? secteur;
  final String? ville;
  final String? siteWeb;
  final ForgeFile? logo;
  final int? satisfaction;
  final Decimal? chiffreAffaires;
  final String? presentation;
  final Decimal? pipeline;
  final Decimal? pipelinePondere;
  final int? nbContacts;
  final int? owner;
  final DateTime createdAt;
  final DateTime updatedAt;

  /// Colonnes modifiables, au format de l'API (création, modification).
  Map<String, dynamic> toJson() => {
    'nom': nom,
    'secteur': secteur?.value,
    'ville': ville,
    'site_web': siteWeb,
    'logo': logo?.id,
    'satisfaction': satisfaction,
    'chiffre_affaires': decimalToJson(chiffreAffaires),
    'presentation': presentation,
  };
}

/// Contact (table `contact`).
class Contact {
  const Contact({
    required this.id,
    this.prenom,
    required this.nom,
    this.email,
    this.telephone,
    this.photo,
    this.poste,
    this.entreprise,
    this.secteur,
    this.notes,
    this.owner,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Contact.fromJson(Map<String, dynamic> json) => Contact(
    id: json['id'] as int,
    prenom: json['prenom'] as String?,
    nom: json['nom'] as String,
    email: json['email'] as String?,
    telephone: json['telephone'] as String?,
    photo: ForgeFile.fromJson(json['photo']),
    poste: json['poste'] as String?,
    entreprise: json['entreprise'] as int?,
    secteur: EntrepriseSecteur.fromJson(json['secteur']),
    notes: json['notes'] as String?,
    owner: json['owner'] as int?,
    createdAt: jsonToDateTime(json['created_at'])!,
    updatedAt: jsonToDateTime(json['updated_at'])!,
  );

  static const table = 'contact';

  /// Opérations typées sur la table.
  static TableClient<Contact> api(ForgeClient client) => TableClient(
    client,
    table,
    Contact.fromJson,
  );

  final int id;
  final String? prenom;
  final String nom;
  final String? email;
  final String? telephone;
  final ForgeFile? photo;
  final String? poste;
  final int? entreprise;
  final EntrepriseSecteur? secteur;
  final String? notes;
  final int? owner;
  final DateTime createdAt;
  final DateTime updatedAt;

  /// Colonnes modifiables, au format de l'API (création, modification).
  Map<String, dynamic> toJson() => {
    'prenom': prenom,
    'nom': nom,
    'email': email,
    'telephone': telephone,
    'photo': photo?.id,
    'poste': poste,
    'entreprise': entreprise,
    'notes': notes,
  };
}

/// Opportunité (table `opportunite`).
class Opportunite {
  const Opportunite({
    required this.id,
    required this.titre,
    required this.entreprise,
    this.contact,
    this.secteur,
    required this.montant,
    this.probabilite,
    this.montantPondere,
    this.montantTtc,
    this.etape,
    this.dateCloture,
    this.joursRestants,
    required this.tags,
    this.devis,
    this.notesInternes,
    this.owner,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Opportunite.fromJson(Map<String, dynamic> json) => Opportunite(
    id: json['id'] as int,
    titre: json['titre'] as String,
    entreprise: json['entreprise'] as int,
    contact: json['contact'] as int?,
    secteur: EntrepriseSecteur.fromJson(json['secteur']),
    montant: jsonToDecimal(json['montant'])!,
    probabilite: json['probabilite'] as int?,
    montantPondere: jsonToDecimal(json['montant_pondere']),
    montantTtc: jsonToDecimal(json['montant_ttc']),
    etape: OpportuniteEtape.fromJson(json['etape']),
    dateCloture: jsonToDate(json['date_cloture']),
    joursRestants: json['jours_restants'] as int?,
    tags: jsonToIds(json['tags']),
    devis: ForgeFile.fromJson(json['devis']),
    notesInternes: json['notes_internes'] as String?,
    owner: json['owner'] as int?,
    createdAt: jsonToDateTime(json['created_at'])!,
    updatedAt: jsonToDateTime(json['updated_at'])!,
  );

  static const table = 'opportunite';

  /// Opérations typées sur la table.
  static TableClient<Opportunite> api(ForgeClient client) => TableClient(
    client,
    table,
    Opportunite.fromJson,
  );

  final int id;
  final String titre;
  final int entreprise;
  final int? contact;
  final EntrepriseSecteur? secteur;
  final Decimal montant;
  final int? probabilite;
  final Decimal? montantPondere;
  final Decimal? montantTtc;
  final OpportuniteEtape? etape;
  final DateTime? dateCloture;
  final int? joursRestants;
  final List<int> tags;
  final ForgeFile? devis;
  final String? notesInternes;
  final int? owner;
  final DateTime createdAt;
  final DateTime updatedAt;

  /// Colonnes modifiables, au format de l'API (création, modification).
  Map<String, dynamic> toJson() => {
    'titre': titre,
    'entreprise': entreprise,
    'contact': contact,
    'montant': decimalToJson(montant),
    'probabilite': probabilite,
    'etape': etape?.value,
    'date_cloture': dateToJson(dateCloture),
    'tags': tags,
    'devis': devis?.id,
    'notes_internes': notesInternes,
  };
}

/// Activité (table `activite`).
class Activite {
  const Activite({
    required this.id,
    required this.sujet,
    required this.nature,
    required this.debut,
    this.duree,
    this.opportunite,
    this.contact,
    this.terminee,
    this.compteRendu,
    this.owner,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Activite.fromJson(Map<String, dynamic> json) => Activite(
    id: json['id'] as int,
    sujet: json['sujet'] as String,
    nature: ActiviteNature.fromJson(json['nature'])!,
    debut: jsonToDateTime(json['debut'])!,
    duree: jsonToDuration(json['duree']),
    opportunite: json['opportunite'] as int?,
    contact: json['contact'] as int?,
    terminee: json['terminee'] as bool?,
    compteRendu: json['compte_rendu'] as String?,
    owner: json['owner'] as int?,
    createdAt: jsonToDateTime(json['created_at'])!,
    updatedAt: jsonToDateTime(json['updated_at'])!,
  );

  static const table = 'activite';

  /// Opérations typées sur la table.
  static TableClient<Activite> api(ForgeClient client) => TableClient(
    client,
    table,
    Activite.fromJson,
  );

  final int id;
  final String sujet;
  final ActiviteNature nature;
  final DateTime debut;
  final Duration? duree;
  final int? opportunite;
  final int? contact;
  final bool? terminee;
  final String? compteRendu;
  final int? owner;
  final DateTime createdAt;
  final DateTime updatedAt;

  /// Colonnes modifiables, au format de l'API (création, modification).
  Map<String, dynamic> toJson() => {
    'sujet': sujet,
    'nature': nature.value,
    'debut': dateTimeToJson(debut),
    'duree': durationToJson(duree),
    'opportunite': opportunite,
    'contact': contact,
    'terminee': terminee,
    'compte_rendu': compteRendu,
  };
}

/// Étiquette (table `tag`).
class Tag {
  const Tag({
    required this.id,
    required this.nom,
    this.couleur,
    this.nbOpportunites,
    this.owner,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Tag.fromJson(Map<String, dynamic> json) => Tag(
    id: json['id'] as int,
    nom: json['nom'] as String,
    couleur: json['couleur'] as String?,
    nbOpportunites: json['nb_opportunites'] as int?,
    owner: json['owner'] as int?,
    createdAt: jsonToDateTime(json['created_at'])!,
    updatedAt: jsonToDateTime(json['updated_at'])!,
  );

  static const table = 'tag';

  /// Opérations typées sur la table.
  static TableClient<Tag> api(ForgeClient client) => TableClient(
    client,
    table,
    Tag.fromJson,
  );

  final int id;
  final String nom;
  final String? couleur;
  final int? nbOpportunites;
  final int? owner;
  final DateTime createdAt;
  final DateTime updatedAt;

  /// Colonnes modifiables, au format de l'API (création, modification).
  Map<String, dynamic> toJson() => {'nom': nom, 'couleur': couleur};
}

enum EntrepriseSecteur {
  industrie('industrie'),
  services('services'),
  commerce('commerce'),
  public('public');

  const EntrepriseSecteur(this.value);

  /// Valeur dans l'API.
  final String value;

  static EntrepriseSecteur? fromJson(Object? json) {
    for (final candidate in values) {
      if (candidate.value == json) return candidate;
    }
    return null;
  }
}

enum OpportuniteEtape {
  prospect('prospect'),
  proposition('proposition'),
  gagne('gagne'),
  perdu('perdu');

  const OpportuniteEtape(this.value);

  /// Valeur dans l'API.
  final String value;

  static OpportuniteEtape? fromJson(Object? json) {
    for (final candidate in values) {
      if (candidate.value == json) return candidate;
    }
    return null;
  }
}

enum ActiviteNature {
  appel('appel'),
  reunion('reunion'),
  email('email'),
  tache('tache');

  const ActiviteNature(this.value);

  /// Valeur dans l'API.
  final String value;

  static ActiviteNature? fromJson(Object? json) {
    for (final candidate in values) {
      if (candidate.value == json) return candidate;
    }
    return null;
  }
}
