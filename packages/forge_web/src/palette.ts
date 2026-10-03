/**
 * Couleurs de l'identité visuelle, communes aux interfaces web et Flutter.
 *
 * La palette catégorielle colore les valeurs d'énumération (badges, barres
 * des statistiques) : chaque valeur garde sa couleur partout, dans l'ordre de
 * déclaration. L'ordre est validé pour les daltonismes (écart entre voisines) ;
 * au-delà de huit valeurs, la couleur neutre est utilisée.
 */
export const CATEGORICAL = {
  light: ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#008300', '#4a3aa7', '#e34948'],
  dark: ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#008300', '#9085e9', '#e66767'],
} as const;

export const NEUTRAL = { light: '#898781', dark: '#898781' } as const;

/** Couleur principale de l'interface (violet indigo). */
export const PRIMARY = '#5b4bd6';

/** Couleur de la valeur d'énumération `value` parmi `values`. */
export function enumColor(
  values: readonly string[] | undefined,
  value: string,
  scheme: 'light' | 'dark',
): string {
  const index = values?.indexOf(value) ?? -1;
  const palette = CATEGORICAL[scheme];
  return index >= 0 && index < palette.length ? palette[index] : NEUTRAL[scheme];
}
