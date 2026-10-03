import { createTheme, mergeThemeOverrides, type MantineThemeOverride } from '@mantine/core';

/**
 * Thème « Material moderne » : couleur principale indigo, coins arrondis,
 * cartes en relief léger, dégradés sur les éléments d'accent.
 */
export const forgeTheme = createTheme({
  primaryColor: 'indigo',
  primaryShade: { light: 6, dark: 5 },
  defaultRadius: 'md',
  fontFamily: "'Inter Variable', Inter, system-ui, -apple-system, 'Segoe UI', sans-serif",
  headings: { fontWeight: '700' },
  defaultGradient: { from: 'indigo', to: 'violet', deg: 135 },
  components: {
    Card: { defaultProps: { shadow: 'sm', radius: 'lg', withBorder: true } },
    Paper: { defaultProps: { radius: 'lg' } },
    Button: { defaultProps: { radius: 'md' } },
    Badge: { defaultProps: { radius: 'sm' } },
  },
});

export const buildTheme = (custom?: MantineThemeOverride) =>
  custom ? mergeThemeOverrides(forgeTheme, custom) : forgeTheme;
