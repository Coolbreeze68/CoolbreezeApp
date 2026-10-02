import 'package:flutter/material.dart';

import '../api/client.dart';
import '../forge.dart';
import 'shell.dart';
import 'widgets.dart';

/// Compte connecté : informations, langue, mot de passe, déconnexion.
class AccountPage extends StatelessWidget {
  const AccountPage({super.key});

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final user = forge.user;
    return ForgeScaffold(
      title: Text(s.account),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Constrained(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (user != null)
                Card(
                  child: ListTile(
                    leading: const Icon(Icons.account_circle, size: 40),
                    title: Text(user.name),
                    subtitle: Text([user.email, ...user.roles].join(' · ')),
                  ),
                ),
              if (forge.schema.locales.length > 1) ...[
                const SizedBox(height: 16),
                Card(
                  child: ListTile(
                    leading: const Icon(Icons.language),
                    title: Text(s.language),
                    trailing: DropdownButton<String>(
                      value: forge.locale,
                      underline: const SizedBox.shrink(),
                      items: [
                        for (final locale in forge.schema.locales)
                          DropdownMenuItem(
                            value: locale,
                            child: Text(languageName(locale)),
                          ),
                      ],
                      onChanged: (locale) {
                        if (locale != null) forge.setLocale(locale);
                      },
                    ),
                  ),
                ),
              ],
              const SizedBox(height: 16),
              const Card(
                child: Padding(
                  padding: EdgeInsets.all(16),
                  child: _PasswordForm(),
                ),
              ),
              const SizedBox(height: 16),
              OutlinedButton.icon(
                onPressed: forge.client.signOut,
                icon: const Icon(Icons.logout),
                label: Text(s.signOut),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Nom d'une langue dans cette langue.
String languageName(String code) => switch (code) {
  'fr' => 'Français',
  'en' => 'English',
  'de' => 'Deutsch',
  'es' => 'Español',
  'it' => 'Italiano',
  'pt' => 'Português',
  'nl' => 'Nederlands',
  _ => code,
};

class _PasswordForm extends StatefulWidget {
  const _PasswordForm();

  @override
  State<_PasswordForm> createState() => _PasswordFormState();
}

class _PasswordFormState extends State<_PasswordForm> {
  final _form = GlobalKey<FormState>();
  final _current = TextEditingController();
  final _new = TextEditingController();
  final _confirmation = TextEditingController();
  Map<String, List<String>> _errors = const {};
  bool _saving = false;

  @override
  void dispose() {
    _current.dispose();
    _new.dispose();
    _confirmation.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_form.currentState!.validate()) return;
    final forge = Forge.of(context);
    setState(() {
      _saving = true;
      _errors = const {};
    });
    try {
      await forge.client.put('/api/auth/password', {
        'current_password': _current.text,
        'new_password': _new.text,
      });
      // Le changement ferme toutes les sessions : on en rouvre une.
      await forge.client.signIn(forge.user!.email, _new.text);
      if (!mounted) return;
      _form.currentState!.reset();
      for (final c in [_current, _new, _confirmation]) {
        c.clear();
      }
      showMessage(context, forge.strings.passwordChanged);
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() => _errors = error.fields);
      if (error.fields.isEmpty) showError(context, error);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = Forge.of(context).strings;
    String? required(String? v) => v == null || v.isEmpty ? s.required : null;
    String? error(String field) => _errors[field]?.join('\n');
    return Form(
      key: _form,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            s.changePassword,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 8),
          TextFormField(
            controller: _current,
            obscureText: true,
            decoration: InputDecoration(
              labelText: s.currentPassword,
              errorText: error('current_password'),
            ),
            validator: required,
          ),
          TextFormField(
            controller: _new,
            obscureText: true,
            decoration: InputDecoration(
              labelText: s.newPassword,
              errorText: error('password') ?? error('new_password'),
            ),
            validator: required,
          ),
          TextFormField(
            controller: _confirmation,
            obscureText: true,
            decoration: InputDecoration(labelText: s.confirmPassword),
            validator: (v) => v != _new.text ? s.passwordsDiffer : null,
          ),
          const SizedBox(height: 16),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton(
              onPressed: _saving ? null : _submit,
              child: Text(s.save),
            ),
          ),
        ],
      ),
    );
  }
}
