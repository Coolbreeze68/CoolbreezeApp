// NE PAS MODIFIER : code généré par forge depuis `forge.json`.
// Ce dossier est réécrit à chaque `forge generate` ; votre code va dans
// `src/custom/`.

// Types des enregistrements, tels que l'API les envoie, pour le code
// personnalisé :
//   const listing = await entrepriseApi(client).list();

import { type ForgeClient, type ForgeFile, type Json, TableClient } from '@forge/web';

/** Entreprise (table `entreprise`). */
export interface Entreprise {
  id: number;
  nom: string;
  secteur: EntrepriseSecteur | null;
  ville: string | null;
  site_web: string | null;
  logo: ForgeFile | null;
  satisfaction: number | null;
  chiffre_affaires: string | null;
  presentation: string | null;
  pipeline: string | null;
  pipeline_pondere: string | null;
  nb_contacts: number | null;
  owner: number | null;
  created_at: string;
  updated_at: string;
}

export type EntrepriseSecteur = 'industrie' | 'services' | 'commerce' | 'public';

/** Opérations typées sur la table `entreprise`. */
export const entrepriseApi = (client: ForgeClient) =>
  new TableClient<Entreprise>(client, 'entreprise', (json: Json) => json as unknown as Entreprise);

/** Contact (table `contact`). */
export interface Contact {
  id: number;
  prenom: string | null;
  nom: string;
  email: string | null;
  telephone: string | null;
  photo: ForgeFile | null;
  poste: string | null;
  entreprise: number | null;
  secteur: EntrepriseSecteur | null;
  notes: string | null;
  owner: number | null;
  created_at: string;
  updated_at: string;
}

/** Opérations typées sur la table `contact`. */
export const contactApi = (client: ForgeClient) =>
  new TableClient<Contact>(client, 'contact', (json: Json) => json as unknown as Contact);

/** Opportunité (table `opportunite`). */
export interface Opportunite {
  id: number;
  titre: string;
  entreprise: number;
  contact: number | null;
  secteur: EntrepriseSecteur | null;
  montant: string;
  probabilite: number | null;
  montant_pondere: string | null;
  montant_ttc: string | null;
  etape: OpportuniteEtape | null;
  date_cloture: string | null;
  jours_restants: number | null;
  tags: number[];
  devis: ForgeFile | null;
  notes_internes: string | null;
  owner: number | null;
  created_at: string;
  updated_at: string;
}

export type OpportuniteEtape = 'prospect' | 'proposition' | 'gagne' | 'perdu';

/** Opérations typées sur la table `opportunite`. */
export const opportuniteApi = (client: ForgeClient) =>
  new TableClient<Opportunite>(client, 'opportunite', (json: Json) => json as unknown as Opportunite);

/** Activité (table `activite`). */
export interface Activite {
  id: number;
  sujet: string;
  nature: ActiviteNature;
  debut: string;
  duree: number | null;
  opportunite: number | null;
  contact: number | null;
  terminee: boolean | null;
  compte_rendu: string | null;
  owner: number | null;
  created_at: string;
  updated_at: string;
}

export type ActiviteNature = 'appel' | 'reunion' | 'email' | 'tache';

/** Opérations typées sur la table `activite`. */
export const activiteApi = (client: ForgeClient) =>
  new TableClient<Activite>(client, 'activite', (json: Json) => json as unknown as Activite);

/** Étiquette (table `tag`). */
export interface Tag {
  id: number;
  nom: string;
  couleur: string | null;
  nb_opportunites: number | null;
  owner: number | null;
  created_at: string;
  updated_at: string;
}

/** Opérations typées sur la table `tag`. */
export const tagApi = (client: ForgeClient) =>
  new TableClient<Tag>(client, 'tag', (json: Json) => json as unknown as Tag);
