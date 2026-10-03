/// Textes de l'interface. `fr` et `en` sont fournis ; d'autres langues (ou
/// des textes modifiés) se déclarent dans `ForgeCustomization.strings`.
library;

class ForgeStrings {
  const ForgeStrings({
    required this.records,
    required this.sizeUnits,
    required this.write,
    required this.preview,
    required this.chooseFile,
    required this.chooseImage,
    required this.replace,
    required this.remove,
    required this.open,
    required this.uploading,
    required this.chooseColor,
    required this.fileTooLarge,
    required this.welcome,
    required this.dashboard,
    required this.account,
    required this.active,
    required this.add,
    required this.all,
    required this.allDay,
    required this.apply,
    required this.average,
    required this.calendarView,
    required this.cancel,
    required this.changePassword,
    required this.clear,
    required this.confirm,
    required this.confirmDelete,
    required this.confirmPassword,
    required this.contains,
    required this.count,
    required this.create,
    required this.currentPassword,
    required this.delete,
    required this.deleted,
    required this.discard,
    required this.discardChanges,
    required this.displayName,
    required this.durationHint,
    required this.edit,
    required this.editRecord,
    required this.email,
    required this.exportCsv,
    required this.exported,
    required this.filter,
    required this.fixErrors,
    required this.forbidden,
    required this.from,
    required this.importCsv,
    required this.importRejected,
    required this.imported,
    required this.invalidCredentials,
    required this.invalidValue,
    required this.keepPasswordHint,
    required this.language,
    required this.line,
    required this.listView,
    required this.max,
    required this.min,
    required this.minutesUnit,
    required this.more,
    required this.networkError,
    required this.newPassword,
    required this.newRecord,
    required this.newUser,
    required this.nextMonth,
    required this.nextPage,
    required this.no,
    required this.noEvents,
    required this.noResults,
    required this.none,
    required this.notFound,
    required this.ok,
    required this.parameters,
    required this.password,
    required this.passwordChanged,
    required this.passwordsDiffer,
    required this.previousMonth,
    required this.previousPage,
    required this.range,
    required this.required,
    required this.retry,
    required this.roles,
    required this.save,
    required this.search,
    required this.seeAll,
    required this.signIn,
    required this.signOut,
    required this.statsView,
    required this.sum,
    required this.timestamps,
    required this.to,
    required this.today,
    required this.unexpectedError,
    required this.users,
    required this.yes,
  });

  final String dashboard;

  final String welcome;

  final String records;

  /// Textes de `locale`, en anglais si la langue n'est pas fournie.
  factory ForgeStrings.of(String locale) => switch (locale) {
    'fr' => fr,
    _ => en,
  };

  final String account;
  final String active;
  final String add;
  final String all;
  final String allDay;
  final String apply;
  final String average;
  final String calendarView;
  final String cancel;
  final String changePassword;
  final String clear;
  final String confirm;

  /// Confirmation de suppression.
  final String Function(String title) confirmDelete;
  final String confirmPassword;
  final String contains;
  final String count;
  final String create;
  final String currentPassword;
  final String delete;
  final String deleted;
  final String discard;
  final String discardChanges;
  final String displayName;
  final String durationHint;
  final String edit;

  /// Titre du formulaire de modification.
  final String Function(String table) editRecord;
  final String email;
  final String exportCsv;
  final String exported;
  final String filter;
  final String fixErrors;
  final String forbidden;
  final String from;
  final String importCsv;
  final String importRejected;

  /// Bilan d'un import.
  final String Function(int created, int updated) imported;
  final String invalidCredentials;
  final String invalidValue;
  final String keepPasswordHint;
  final String language;

  /// Numéro de ligne d'un fichier.
  final String Function(int line) line;
  final String listView;
  final String max;
  final String min;
  final String minutesUnit;
  final String more;
  final String networkError;
  final String newPassword;

  /// Titre du formulaire de création.
  final String Function(String table) newRecord;
  final String newUser;
  final String nextMonth;
  final String nextPage;
  final String no;
  final String noEvents;
  final String noResults;
  final String none;
  final String notFound;
  final String ok;
  final String parameters;
  final String password;
  final String passwordChanged;
  final String passwordsDiffer;
  final String previousMonth;
  final String previousPage;

  /// Position dans une liste paginée.
  final String Function(int first, int last, int total) range;
  final String required;
  final String retry;
  final String roles;
  final String save;
  final String search;
  final String seeAll;
  final String signIn;
  final String signOut;
  final String statsView;
  final String sum;

  /// Dates de création et de modification.
  final String Function(String created, String updated) timestamps;
  final String to;
  final String today;
  final String unexpectedError;
  final String users;
  final String yes;

  /// Unités de taille de fichier, croissantes.
  final List<String> sizeUnits;
  final String write;
  final String preview;
  final String chooseFile;
  final String chooseImage;
  final String replace;
  final String remove;
  final String open;
  final String uploading;
  final String chooseColor;

  /// Fichier refusé avant envoi.
  final String Function(int megabytes) fileTooLarge;
}

const fr = ForgeStrings(
  sizeUnits: ['o', 'Ko', 'Mo', 'Go'],
  write: 'Écrire',
  preview: 'Aperçu',
  chooseFile: 'Choisir un fichier',
  chooseImage: 'Choisir une image',
  replace: 'Remplacer',
  remove: 'Retirer',
  open: 'Ouvrir',
  uploading: 'Envoi…',
  chooseColor: 'Choisir une couleur',
  fileTooLarge: _frFileTooLarge,
  records: 'Enregistrements',
  welcome: 'Bonjour',
  dashboard: 'Tableau de bord',
  account: 'Mon compte',
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
  confirmDelete: _frConfirmDelete,
  confirmPassword: 'Confirmation du mot de passe',
  contains: 'Contient',
  count: 'Nombre',
  create: 'Nouveau',
  currentPassword: 'Mot de passe actuel',
  delete: 'Supprimer',
  deleted: 'Supprimé',
  discard: 'Abandonner',
  discardChanges: 'Abandonner les modifications ?',
  displayName: 'Nom affiché',
  durationHint: 'h:mm, ou un nombre de minutes',
  edit: 'Modifier',
  editRecord: _frEditRecord,
  email: 'E-mail',
  exportCsv: 'Exporter (CSV)',
  exported: 'Export enregistré',
  filter: 'Filtrer',
  fixErrors: 'Corrigez les erreurs signalées',
  forbidden: 'Action non autorisée',
  from: 'Du',
  importCsv: 'Importer (CSV)',
  importRejected: 'Import refusé : rien n\'a été enregistré',
  imported: _frImported,
  invalidCredentials: 'E-mail ou mot de passe incorrect',
  invalidValue: 'Valeur invalide',
  keepPasswordHint: 'Laisser vide pour ne pas le changer',
  language: 'Langue',
  line: _frLine,
  listView: 'Liste',
  max: 'max.',
  min: 'min.',
  minutesUnit: 'min',
  more: 'Plus',
  networkError: 'Serveur injoignable',
  newPassword: 'Nouveau mot de passe',
  newRecord: _frNewRecord,
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
  range: _frRange,
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
  timestamps: _frTimestamps,
  to: 'Au',
  today: 'Aujourd\'hui',
  unexpectedError: 'Erreur inattendue',
  users: 'Utilisateurs',
  yes: 'Oui',
);

String _frConfirmDelete(String title) => 'Supprimer « $title » ?';

String _frEditRecord(String table) => 'Modifier : $table';

String _frNewRecord(String table) => 'Nouveau : $table';

String _frImported(int created, int updated) =>
    'Import terminé : $created créé(s), $updated modifié(s)';

String _frLine(int line) => 'Ligne $line';

String _frRange(int first, int last, int total) => '$first–$last sur $total';

String _frTimestamps(String created, String updated) =>
    'Créé le $created · modifié le $updated';

const en = ForgeStrings(
  sizeUnits: ['B', 'KB', 'MB', 'GB'],
  write: 'Write',
  preview: 'Preview',
  chooseFile: 'Choose a file',
  chooseImage: 'Choose an image',
  replace: 'Replace',
  remove: 'Remove',
  open: 'Open',
  uploading: 'Uploading…',
  chooseColor: 'Choose a color',
  fileTooLarge: _enFileTooLarge,
  records: 'Records',
  welcome: 'Hello',
  dashboard: 'Dashboard',
  account: 'My account',
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
  confirmDelete: _enConfirmDelete,
  confirmPassword: 'Confirm password',
  contains: 'Contains',
  count: 'Count',
  create: 'New',
  currentPassword: 'Current password',
  delete: 'Delete',
  deleted: 'Deleted',
  discard: 'Discard',
  discardChanges: 'Discard your changes?',
  displayName: 'Display name',
  durationHint: 'h:mm, or a number of minutes',
  edit: 'Edit',
  editRecord: _enEditRecord,
  email: 'Email',
  exportCsv: 'Export (CSV)',
  exported: 'Export saved',
  filter: 'Filter',
  fixErrors: 'Please fix the errors',
  forbidden: 'Not allowed',
  from: 'From',
  importCsv: 'Import (CSV)',
  importRejected: 'Import rejected: nothing was saved',
  imported: _enImported,
  invalidCredentials: 'Wrong email or password',
  invalidValue: 'Invalid value',
  keepPasswordHint: 'Leave empty to keep it',
  language: 'Language',
  line: _enLine,
  listView: 'List',
  max: 'max.',
  min: 'min.',
  minutesUnit: 'min',
  more: 'More',
  networkError: 'Server unreachable',
  newPassword: 'New password',
  newRecord: _enNewRecord,
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
  range: _enRange,
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
  timestamps: _enTimestamps,
  to: 'To',
  today: 'Today',
  unexpectedError: 'Unexpected error',
  users: 'Users',
  yes: 'Yes',
);

String _enConfirmDelete(String title) => 'Delete "$title"?';

String _enEditRecord(String table) => 'Edit: $table';

String _enNewRecord(String table) => 'New: $table';

String _enImported(int created, int updated) =>
    'Import done: $created created, $updated updated';

String _enLine(int line) => 'Line $line';

String _enRange(int first, int last, int total) => '$first–$last of $total';

String _enTimestamps(String created, String updated) =>
    'Created $created · updated $updated';

String _frFileTooLarge(int megabytes) =>
    'Fichier trop volumineux ($megabytes Mo au maximum)';

String _enFileTooLarge(int megabytes) =>
    'File too large ($megabytes MB maximum)';
