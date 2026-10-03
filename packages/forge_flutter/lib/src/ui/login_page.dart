import 'package:flutter/material.dart';

import '../api/client.dart';
import '../forge.dart';
import '../theme.dart';
import 'widgets.dart';

class LoginPage extends StatefulWidget {
  const LoginPage({super.key});

  @override
  State<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends State<LoginPage> {
  final _form = GlobalKey<FormState>();
  final _email = TextEditingController();
  final _password = TextEditingController();
  String? _error;
  bool _busy = false;

  @override
  void dispose() {
    _email.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_form.currentState!.validate()) return;
    final forge = Forge.of(context);
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      // La redirection suit l'ouverture de la session.
      await forge.client.signIn(_email.text.trim(), _password.text);
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(
        () => _error = error.isUnauthorized
            ? forge.strings.invalidCredentials
            : errorMessage(context, error),
      );
    } catch (error) {
      // Erreur imprévue : affichée plutôt qu'ignorée.
      if (mounted) setState(() => _error = errorMessage(context, error));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    String? required(String? value) =>
        value == null || value.trim().isEmpty ? s.required : null;
    final form = Form(
      key: _form,
      child: AutofillGroup(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Center(
              child: Container(
                width: 56,
                height: 56,
                decoration: const BoxDecoration(
                  gradient: ForgeColors.gradient,
                  shape: BoxShape.circle,
                ),
                child: const Icon(Icons.lock_outline, color: Colors.white),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              forge.schema.name,
              style: Theme.of(
                context,
              ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 24),
            TextFormField(
              controller: _email,
              decoration: InputDecoration(labelText: s.email),
              keyboardType: TextInputType.emailAddress,
              autofillHints: const [AutofillHints.email],
              validator: required,
            ),
            const SizedBox(height: 16),
            TextFormField(
              controller: _password,
              decoration: InputDecoration(labelText: s.password),
              obscureText: true,
              autofillHints: const [AutofillHints.password],
              validator: required,
              onFieldSubmitted: (_) => _submit(),
            ),
            if (_error != null) ...[
              const SizedBox(height: 16),
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 24),
            FilledButton(
              onPressed: _busy ? null : _submit,
              style: FilledButton.styleFrom(
                padding: const EdgeInsets.symmetric(vertical: 16),
              ),
              child: Text(s.signIn),
            ),
          ],
        ),
      ),
    );
    return Scaffold(
      body: DecoratedBox(
        decoration: const BoxDecoration(gradient: ForgeColors.vividGradient),
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Card(
                elevation: 8,
                child: Padding(padding: const EdgeInsets.all(28), child: form),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
