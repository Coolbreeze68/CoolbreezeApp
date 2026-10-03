import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:go_router/go_router.dart';

import 'api/client.dart';
import 'customization.dart';
import 'forge.dart';
import 'l10n/strings.dart';
import 'router.dart';
import 'schema.dart';
import 'theme.dart';
import 'values.dart';

const _apiUrl = String.fromEnvironment('FORGE_API_URL');

/// Adresse de l'API : `--dart-define=FORGE_API_URL=…`, sinon l'origine de la
/// page sur le web, sinon `http://localhost:8080`.
Uri defaultApiUrl() {
  if (_apiUrl.isNotEmpty) return Uri.parse(_apiUrl);
  if (kIsWeb) return Uri.parse(Uri.base.origin);
  return Uri.parse('http://localhost:8080');
}

/// `pt_BR` → `Locale('pt', 'BR')`.
Locale _flutterLocale(String code) {
  final [language, ...region] = code.split('_');
  return Locale(language, region.firstOrNull);
}

/// Application complète décrite par `schema`.
class ForgeApp extends StatefulWidget {
  const ForgeApp({
    super.key,
    required this.schema,
    required this.client,
    this.customization = const ForgeCustomization(),
  });

  final AppSchema schema;
  final ForgeClient client;
  final ForgeCustomization customization;

  @override
  State<ForgeApp> createState() => _ForgeAppState();
}

class _ForgeAppState extends State<ForgeApp> {
  /// Clé de la langue choisie dans le stockage du client.
  static const _localeKey = 'forge.locale';

  late final _changes = DataChanges();
  late final GoRouter _router = buildRouter(widget.schema, widget.client);
  late String _locale = widget.schema.defaultLocale;
  late ValueFormat _format;
  late TitleCache _titles;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _configure();
    widget.client.addListener(_sessionChanged);
    _changes.addListener(_dataChanged);
    _start();
  }

  Future<void> _start() async {
    final saved = await widget.client.store.read(_localeKey);
    if (saved != null && widget.schema.locales.contains(saved)) _locale = saved;
    await widget.client.restore();
    if (!mounted) return;
    setState(() {
      _configure();
      _ready = true;
    });
  }

  void _configure() {
    final custom = widget.customization.strings;
    _format = ValueFormat(
      locale: _locale,
      fallbackLocale: widget.schema.defaultLocale,
      strings: custom[_locale] ?? ForgeStrings.of(_locale),
      enumLabels: widget.customization.enumLabels,
    );
    _titles = TitleCache(widget.client, widget.schema, _format);
  }

  void _sessionChanged() => setState(_titles.clear);

  void _dataChanged() => _titles.clear();

  void _setLocale(String locale) {
    setState(() {
      _locale = locale;
      _configure();
    });
    widget.client.store.write(_localeKey, locale);
  }

  @override
  void dispose() {
    widget.client.removeListener(_sessionChanged);
    _changes.dispose();
    _router.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final custom = widget.customization;
    final theme = custom.theme ?? forgeTheme();
    final darkTheme =
        custom.darkTheme ?? forgeTheme(brightness: Brightness.dark);
    if (!_ready) {
      return MaterialApp(
        theme: theme,
        darkTheme: darkTheme,
        debugShowCheckedModeBanner: false,
        home: const Scaffold(body: Center(child: CircularProgressIndicator())),
      );
    }
    return MaterialApp.router(
      title: widget.schema.name,
      theme: theme,
      darkTheme: darkTheme,
      debugShowCheckedModeBanner: false,
      locale: _flutterLocale(_locale),
      supportedLocales: [
        for (final l in widget.schema.locales) _flutterLocale(l),
      ],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      routerConfig: _router,
      builder: (context, child) => Forge(
        schema: widget.schema,
        client: widget.client,
        customization: custom,
        format: _format,
        titles: _titles,
        changes: _changes,
        user: widget.client.user,
        locale: _locale,
        setLocale: _setLocale,
        child: child!,
      ),
    );
  }
}
