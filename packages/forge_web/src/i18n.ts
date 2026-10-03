/** Textes de l'interface. `fr` et `en` sont fournis ; d'autres langues (ou des
 * textes modifiés) se déclarent dans `ForgeCustomization.strings`. */

export interface ForgeStrings {
  account: string;
  actions: string;
  active: string;
  add: string;
  all: string;
  allDay: string;
  apply: string;
  average: string;
  calendarView: string;
  cancel: string;
  changePassword: string;
  clear: string;
  confirm: string;
  confirmDelete: (title: string) => string;
  confirmPassword: string;
  contains: string;
  count: string;
  create: string;
  currentPassword: string;
  dark: string;
  dashboard: string;
  delete: string;
  deleted: string;
  details: string;
  discard: string;
  discardChanges: string;
  displayName: string;
  durationHint: string;
  edit: string;
  editRecord: (table: string) => string;
  email: string;
  exportCsv: string;
  exported: string;
  filter: string;
  fixErrors: string;
  forbidden: string;
  from: string;
  home: string;
  importCsv: string;
  importRejected: string;
  imported: (created: number, updated: number) => string;
  invalidCredentials: string;
  invalidValue: string;
  keepPasswordHint: string;
  language: string;
  light: string;
  line: (line: number) => string;
  listView: string;
  loading: string;
  max: string;
  min: string;
  minutesUnit: string;
  more: string;
  networkError: string;
  newPassword: string;
  newRecord: (table: string) => string;
  newUser: string;
  nextMonth: string;
  nextPage: string;
  no: string;
  noEvents: string;
  noResults: string;
  none: string;
  notFound: string;
  ok: string;
  parameters: string;
  password: string;
  passwordChanged: string;
  passwordsDiffer: string;
  previousMonth: string;
  previousPage: string;
  range: (first: number, last: number, total: number) => string;
  records: string;
  required: string;
  retry: string;
  roles: string;
  save: string;
  search: string;
  seeAll: string;
  signIn: string;
  signOut: string;
  statsView: string;
  sum: string;
  system: string;
  theme: string;
  timestamps: (created: string, updated: string) => string;
  to: string;
  today: string;
  unexpectedError: string;
  users: string;
  welcome: string;
  yes: string;
}

export const fr: ForgeStrings = {
  account: 'Mon compte',
  actions: 'Actions',
  active: 'Actif',
  add: 'Ajouter',
  all: 'Tous',
  allDay: 'Journée',
  apply: 'Appliquer',
  average: 'moy.',
  calendarView: 'Calendrier',
  cancel: 'Annuler',
  changePassword: 'Changer de mot de passe',
  clear: 'Effacer',
  confirm: 'Confirmer',
  confirmDelete: (title: string) => `Supprimer « ${title} » ?`,
  confirmPassword: 'Confirmation du mot de passe',
  contains: 'Contient',
  count: 'Nombre',
  create: 'Nouveau',
  currentPassword: 'Mot de passe actuel',
  dark: 'Sombre',
  dashboard: 'Tableau de bord',
  delete: 'Supprimer',
  deleted: 'Supprimé',
  details: 'Détails',
  discard: 'Abandonner',
  discardChanges: 'Abandonner les modifications ?',
  displayName: 'Nom affiché',
  durationHint: 'h:mm, ou un nombre de minutes',
  edit: 'Modifier',
  editRecord: (table: string) => `Modifier : ${table}`,
  email: 'E-mail',
  exportCsv: 'Exporter (CSV)',
  exported: 'Export enregistré',
  filter: 'Filtrer',
  fixErrors: 'Corrigez les erreurs signalées',
  forbidden: 'Action non autorisée',
  from: 'Du',
  home: 'Accueil',
  importCsv: 'Importer (CSV)',
  importRejected: 'Import refusé : rien n\'a été enregistré',
  imported: (created: number, updated: number) => `Import terminé : ${created} créé(s), ${updated} modifié(s)`,
  invalidCredentials: 'E-mail ou mot de passe incorrect',
  invalidValue: 'Valeur invalide',
  keepPasswordHint: 'Laisser vide pour ne pas le changer',
  language: 'Langue',
  light: 'Clair',
  line: (line: number) => `Ligne ${line}`,
  listView: 'Liste',
  loading: 'Chargement…',
  max: 'max.',
  min: 'min.',
  minutesUnit: 'min',
  more: 'Plus',
  networkError: 'Serveur injoignable',
  newPassword: 'Nouveau mot de passe',
  newRecord: (table: string) => `Nouveau : ${table}`,
  newUser: 'Nouvel utilisateur',
  nextMonth: 'Mois suivant',
  nextPage: 'Page suivante',
  no: 'Non',
  noEvents: 'Rien ce jour-là',
  noResults: 'Aucun résultat',
  none: '(vide)',
  notFound: 'Introuvable',
  ok: 'OK',
  parameters: 'Paramètres',
  password: 'Mot de passe',
  passwordChanged: 'Mot de passe changé',
  passwordsDiffer: 'Les mots de passe diffèrent',
  previousMonth: 'Mois précédent',
  previousPage: 'Page précédente',
  range: (first: number, last: number, total: number) => `${first}–${last} sur ${total}`,
  records: 'enregistrements',
  required: 'Valeur obligatoire',
  retry: 'Réessayer',
  roles: 'Rôles',
  save: 'Enregistrer',
  search: 'Rechercher',
  seeAll: 'Tout voir',
  signIn: 'Se connecter',
  signOut: 'Se déconnecter',
  statsView: 'Statistiques',
  sum: 'total',
  system: 'Système',
  theme: 'Thème',
  timestamps: (created: string, updated: string) => `Créé le ${created} · modifié le ${updated}`,
  to: 'Au',
  today: 'Aujourd\'hui',
  unexpectedError: 'Erreur inattendue',
  users: 'Utilisateurs',
  welcome: 'Bonjour',
  yes: 'Oui',
};

export const en: ForgeStrings = {
  account: 'My account',
  actions: 'Actions',
  active: 'Active',
  add: 'Add',
  all: 'All',
  allDay: 'All day',
  apply: 'Apply',
  average: 'avg.',
  calendarView: 'Calendar',
  cancel: 'Cancel',
  changePassword: 'Change password',
  clear: 'Clear',
  confirm: 'Confirm',
  confirmDelete: (title: string) => `Delete "${title}"?`,
  confirmPassword: 'Confirm password',
  contains: 'Contains',
  count: 'Count',
  create: 'New',
  currentPassword: 'Current password',
  dark: 'Dark',
  dashboard: 'Dashboard',
  delete: 'Delete',
  deleted: 'Deleted',
  details: 'Details',
  discard: 'Discard',
  discardChanges: 'Discard your changes?',
  displayName: 'Display name',
  durationHint: 'h:mm, or a number of minutes',
  edit: 'Edit',
  editRecord: (table: string) => `Edit: ${table}`,
  email: 'Email',
  exportCsv: 'Export (CSV)',
  exported: 'Export saved',
  filter: 'Filter',
  fixErrors: 'Please fix the errors',
  forbidden: 'Not allowed',
  from: 'From',
  home: 'Home',
  importCsv: 'Import (CSV)',
  importRejected: 'Import rejected: nothing was saved',
  imported: (created: number, updated: number) => `Import done: ${created} created, ${updated} updated`,
  invalidCredentials: 'Wrong email or password',
  invalidValue: 'Invalid value',
  keepPasswordHint: 'Leave empty to keep it',
  language: 'Language',
  light: 'Light',
  line: (line: number) => `Line ${line}`,
  listView: 'List',
  loading: 'Loading…',
  max: 'max.',
  min: 'min.',
  minutesUnit: 'min',
  more: 'More',
  networkError: 'Server unreachable',
  newPassword: 'New password',
  newRecord: (table: string) => `New: ${table}`,
  newUser: 'New user',
  nextMonth: 'Next month',
  nextPage: 'Next page',
  no: 'No',
  noEvents: 'Nothing on this day',
  noResults: 'No results',
  none: '(empty)',
  notFound: 'Not found',
  ok: 'OK',
  parameters: 'Settings',
  password: 'Password',
  passwordChanged: 'Password changed',
  passwordsDiffer: 'Passwords do not match',
  previousMonth: 'Previous month',
  previousPage: 'Previous page',
  range: (first: number, last: number, total: number) => `${first}–${last} of ${total}`,
  records: 'records',
  required: 'Required',
  retry: 'Retry',
  roles: 'Roles',
  save: 'Save',
  search: 'Search',
  seeAll: 'See all',
  signIn: 'Sign in',
  signOut: 'Sign out',
  statsView: 'Statistics',
  sum: 'total',
  system: 'System',
  theme: 'Theme',
  timestamps: (created: string, updated: string) => `Created ${created} · updated ${updated}`,
  to: 'To',
  today: 'Today',
  unexpectedError: 'Unexpected error',
  users: 'Users',
  welcome: 'Hello',
  yes: 'Yes',
};

/** Textes de `locale`, en anglais si la langue n'est pas fournie. */
export const stringsFor = (locale: string): ForgeStrings => (locale.startsWith('fr') ? fr : en);
