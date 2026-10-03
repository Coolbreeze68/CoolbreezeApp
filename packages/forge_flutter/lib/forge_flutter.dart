/// Bibliothèque des applications Flutter générées par forge.
///
/// L'application générée fournit sa description des tables
/// (`lib/generated/schema.dart`) et ses personnalisations
/// (`lib/custom/customization.dart`) à [ForgeApp] ; le reste vit ici.
library;

export 'package:go_router/go_router.dart' show GoRouter, GoRouterHelper;

export 'src/api/client.dart';
export 'src/api/query.dart';
export 'src/api/store.dart';
export 'src/api/table_client.dart';
export 'src/app.dart' show ForgeApp, defaultApiUrl;
export 'src/customization.dart';
export 'src/forge.dart';
export 'src/l10n/strings.dart';
export 'src/router.dart' show Paths;
export 'src/palette.dart';
export 'src/schema.dart';
export 'src/theme.dart';
export 'src/ui/fields.dart' show InvalidInput, buildField;
export 'src/ui/record_list.dart' show RecordList;
export 'src/ui/shell.dart' show ForgeScaffold, TintedIcon;
export 'src/ui/stats_view.dart' show GroupBars, StatTile;
export 'src/ui/widgets.dart'
    show
        EnumBadge,
        RecordTitle,
        ValueView,
        errorMessage,
        showError,
        showMessage;
export 'src/values.dart';
