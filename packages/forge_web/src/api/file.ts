/**
 * Fichier téléversé, tel que lu dans un enregistrement (colonnes `file` et
 * `image`). `url` est un lien temporaire, utilisable sans jeton (balise `<img>`).
 */
export interface ForgeFile {
  id: string;
  name: string;
  /** Taille en octets. */
  size: number;
  content_type: string;
  url: string;
}

export const isForgeFile = (value: unknown): value is ForgeFile =>
  typeof value === 'object' && value !== null && typeof (value as ForgeFile).id === 'string' && 'url' in value;
