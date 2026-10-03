/** Chemins de l'application. */
export const paths = {
  home: '/',
  login: '/login',
  account: '/account',
  parameters: '/settings/parameters',
  users: '/settings/users',
  newUser: '/settings/users/new',
  user: (id: number) => `/settings/users/${id}`,
  table: (table: string, filters: Record<string, string> = {}) => {
    const search = new URLSearchParams(filters).toString();
    return `/data/${table}${search ? `?${search}` : ''}`;
  },
  record: (table: string, id: number) => `/data/${table}/${id}`,
  edit: (table: string, id: number) => `/data/${table}/${id}/edit`,
  /** Création, valeurs initiales passées en paramètres d'URL (format de l'API). */
  create: (table: string, initial: Record<string, string> = {}) => {
    const search = new URLSearchParams(initial).toString();
    return `/data/${table}/new${search ? `?${search}` : ''}`;
  },
  page: (path: string) => `/pages/${path}`,
};
