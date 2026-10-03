/// Affichage et saisie des modèles de champ : couleur, liens (e-mail, web,
/// téléphone), note, Markdown, fichier et image.
library;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:go_router/go_router.dart';
import 'package:url_launcher/url_launcher.dart';

import '../api/client.dart';
import '../api/file.dart';
import '../customization.dart';
import '../forge.dart';
import '../palette.dart';
import '../router.dart';
import '../schema.dart';
import 'widgets.dart';

// ------------------------------------------------------------------ couleur

/// `#rrggbb` → couleur.
Color? parseColor(Object? json) {
  if (json is! String || !RegExp(r'^#[0-9a-fA-F]{6}$').hasMatch(json)) {
    return null;
  }
  return Color(0xFF000000 | int.parse(json.substring(1), radix: 16));
}

/// Couleur → `#rrggbb`.
String colorToHex(Color color) =>
    '#${(color.toARGB32() & 0xFFFFFF).toRadixString(16).padLeft(6, '0')}';

/// Pastille de couleur et code hexadécimal.
class ColorValue extends StatelessWidget {
  const ColorValue(this.hex, {super.key});

  final String hex;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      _Swatch(parseColor(hex) ?? Palette.neutral, size: 18),
      const SizedBox(width: 8),
      Text(
        hex,
        style: const TextStyle(fontFeatures: [FontFeature.tabularFigures()]),
      ),
    ],
  );
}

class _Swatch extends StatelessWidget {
  const _Swatch(this.color, {this.size = 28, this.selected = false});

  final Color color;
  final double size;
  final bool selected;

  @override
  Widget build(BuildContext context) => Container(
    width: size,
    height: size,
    decoration: BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(size / 4),
      border: Border.all(color: Theme.of(context).colorScheme.outlineVariant),
    ),
    child: selected
        ? Icon(
            Icons.check,
            size: size * 0.6,
            color: color.computeLuminance() > 0.5 ? Colors.black : Colors.white,
          )
        : null,
  );
}

/// Couleurs proposées : palette de forge, puis teintes Material.
final _presets = [
  ...Palette.light,
  for (final swatch in Colors.primaries) swatch.shade500,
  Colors.black,
  Colors.grey.shade600,
  Colors.white,
];

/// Choix d'une couleur : nuancier et saisie hexadécimale.
Future<String?> pickColor(BuildContext context, String? current) =>
    showDialog<String>(
      context: context,
      builder: (context) => _ColorDialog(current),
    );

class _ColorDialog extends StatefulWidget {
  const _ColorDialog(this.current);

  final String? current;

  @override
  State<_ColorDialog> createState() => _ColorDialogState();
}

class _ColorDialogState extends State<_ColorDialog> {
  late String? _hex = widget.current;
  late final _input = TextEditingController(
    text: widget.current?.substring(1) ?? '',
  );

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  void _select(String hex) => setState(() {
    _hex = hex;
    _input.text = hex.substring(1);
  });

  @override
  Widget build(BuildContext context) {
    final s = Forge.of(context).strings;
    final valid = parseColor(_hex) != null;
    return AlertDialog(
      title: Text(s.chooseColor),
      content: SizedBox(
        width: 340,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final color in _presets)
                  InkWell(
                    borderRadius: BorderRadius.circular(8),
                    onTap: () => _select(colorToHex(color)),
                    child: _Swatch(
                      color,
                      selected: _hex?.toLowerCase() == colorToHex(color),
                    ),
                  ),
              ],
            ),
            const SizedBox(height: 16),
            Row(
              children: [
                _Swatch(parseColor(_hex) ?? Colors.transparent, size: 40),
                const SizedBox(width: 12),
                Expanded(
                  child: TextField(
                    controller: _input,
                    decoration: const InputDecoration(prefixText: '#'),
                    maxLength: 6,
                    buildCounter: _noCounter,
                    onChanged: (text) => setState(() => _hex = '#$text'),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(s.cancel),
        ),
        FilledButton(
          onPressed: valid
              ? () => Navigator.pop(context, _hex!.toLowerCase())
              : null,
          child: Text(s.ok),
        ),
      ],
    );
  }
}

Widget? _noCounter(
  BuildContext context, {
  required int currentLength,
  required bool isFocused,
  required int? maxLength,
}) => null;

// -------------------------------------------------------------------- liens

/// Adresse ouverte par le système : `mailto:`, `tel:` ou adresse web.
Uri? linkUri(ColumnType type, String value) => switch (type) {
  ColumnType.email => Uri(scheme: 'mailto', path: value),
  ColumnType.phone => Uri(
    scheme: 'tel',
    path: value.replaceAll(RegExp(r'[^\d+]'), ''),
  ),
  ColumnType.url => Uri.tryParse(value),
  _ => null,
};

Future<void> openLink(Uri uri) =>
    launchUrl(uri, mode: LaunchMode.externalApplication);

/// E-mail, adresse web ou téléphone, cliquable.
class LinkValue extends StatelessWidget {
  const LinkValue({super.key, required this.type, required this.value});

  final ColumnType type;
  final String value;

  @override
  Widget build(BuildContext context) {
    final uri = linkUri(type, value);
    final icon = switch (type) {
      ColumnType.email => Icons.mail_outline,
      ColumnType.phone => Icons.phone_outlined,
      _ => Icons.open_in_new,
    };
    final color = Theme.of(context).colorScheme.primary;
    return InkWell(
      onTap: uri == null ? null : () => openLink(uri),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 16, color: color),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              value,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: color),
            ),
          ),
        ],
      ),
    );
  }
}

// --------------------------------------------------------------------- note

/// Étoiles d'une note ; modifiable si [onChanged] est fourni.
class RatingStars extends StatelessWidget {
  const RatingStars({
    super.key,
    required this.value,
    required this.max,
    this.onChanged,
    this.size = 20,
  });

  final int value;
  final int max;
  final ValueChanged<int>? onChanged;
  final double size;

  static const color = Color(0xFFF5A524);

  @override
  Widget build(BuildContext context) {
    final empty = Theme.of(context).colorScheme.outlineVariant;
    return Semantics(
      label: '$value/$max',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var star = 1; star <= max; star++)
            GestureDetector(
              onTap: onChanged == null
                  ? null
                  : () => onChanged!(star == value ? star - 1 : star),
              child: Padding(
                padding: const EdgeInsets.all(1),
                child: Icon(
                  star <= value
                      ? Icons.star_rounded
                      : Icons.star_outline_rounded,
                  size: size,
                  color: star <= value ? color : empty,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

// ----------------------------------------------------------------- Markdown

class MarkdownValue extends StatelessWidget {
  const MarkdownValue(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) => MarkdownBody(
    data: text,
    selectable: true,
    onTapLink: (_, href, _) {
      final uri = href == null ? null : Uri.tryParse(href);
      if (uri != null) openLink(uri);
    },
  );
}

// ------------------------------------------------------------------ fichiers

/// Fichier : icône, nom et taille ; ouvert par le navigateur ou le système.
class FileValue extends StatelessWidget {
  const FileValue(this.file, {super.key, this.links = true});

  final ForgeFile file;
  final bool links;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final icon = switch (file.contentType) {
      'application/pdf' => Icons.picture_as_pdf_outlined,
      final type when type.startsWith('image/') => Icons.image_outlined,
      final type when type.startsWith('text/') => Icons.description_outlined,
      _ => Icons.insert_drive_file_outlined,
    };
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 18, color: Theme.of(context).colorScheme.primary),
        const SizedBox(width: 6),
        Flexible(child: Text(file.name, overflow: TextOverflow.ellipsis)),
        const SizedBox(width: 6),
        Text(
          forge.format.fileSize(file.size),
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    );
    if (!links) return content;
    return InkWell(
      onTap: () => openLink(forge.client.resolve(file.url)),
      child: content,
    );
  }
}

/// Miniature d'une image ; agrandie au toucher.
class ImageValue extends StatelessWidget {
  const ImageValue(this.file, {super.key, this.size = 40, this.zoom = true});

  final ForgeFile file;
  final double size;
  final bool zoom;

  @override
  Widget build(BuildContext context) {
    final url = Forge.of(context).client.resolve(file.url).toString();
    final image = ClipRRect(
      borderRadius: BorderRadius.circular(size / 8),
      child: Image.network(
        url,
        width: size,
        height: size,
        fit: BoxFit.cover,
        semanticLabel: file.name,
        errorBuilder: (_, _, _) => SizedBox.square(
          dimension: size,
          child: const Icon(Icons.broken_image_outlined),
        ),
      ),
    );
    // Taille propre, même dans une colonne qui impose sa largeur (fiche).
    return Align(
      alignment: AlignmentDirectional.centerStart,
      widthFactor: 1,
      child: zoom ? _zoomable(context, url, image) : image,
    );
  }

  Widget _zoomable(BuildContext context, String url, Widget image) {
    return InkWell(
      onTap: () => showDialog<void>(
        context: context,
        builder: (context) => Dialog(
          clipBehavior: Clip.antiAlias,
          child: InteractiveViewer(
            child: Image.network(url, semanticLabel: file.name),
          ),
        ),
      ),
      child: image,
    );
  }
}

/// Type MIME d'un fichier choisi, d'après son extension.
String contentTypeOf(String name) {
  final extension = name.contains('.')
      ? name.split('.').last.toLowerCase()
      : '';
  return switch (extension) {
    'pdf' => 'application/pdf',
    'png' => 'image/png',
    'jpg' || 'jpeg' => 'image/jpeg',
    'gif' => 'image/gif',
    'webp' => 'image/webp',
    'svg' => 'image/svg+xml',
    'csv' => 'text/csv',
    'txt' || 'md' => 'text/plain',
    'json' => 'application/json',
    'zip' => 'application/zip',
    'doc' => 'application/msword',
    'docx' =>
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'xls' => 'application/vnd.ms-excel',
    'xlsx' =>
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    'ppt' => 'application/vnd.ms-powerpoint',
    'pptx' =>
      'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    'odt' => 'application/vnd.oasis.opendocument.text',
    'ods' => 'application/vnd.oasis.opendocument.spreadsheet',
    _ => 'application/octet-stream',
  };
}

/// Champ fichier ou image : choix, envoi immédiat, remplacement, retrait.
class FileField extends StatefulWidget {
  const FileField(this.field, {super.key});

  final FieldState field;

  @override
  State<FileField> createState() => _FileFieldState();
}

class _FileFieldState extends State<FileField> {
  bool _uploading = false;

  ColumnSchema get _column => widget.field.column;

  Future<void> _choose() async {
    final forge = Forge.of(context);
    final image = _column.type == ColumnType.image;
    // Le sélecteur ne filtre que des extensions ; les types MIME sont
    // vérifiés par le serveur.
    final extensions = [
      for (final pattern in _column.accept)
        if (pattern.startsWith('.')) pattern.substring(1),
    ];
    final filtered = !image && extensions.length == _column.accept.length;
    final picked = await FilePicker.pickFile(
      type: image
          ? FileType.image
          : filtered && extensions.isNotEmpty
          ? FileType.custom
          : FileType.any,
      allowedExtensions: filtered && extensions.isNotEmpty ? extensions : null,
    );
    if (picked == null || !mounted) return;
    final bytes = await picked.readAsBytes();
    final maxSize = _column.maxSize;
    if (maxSize != null && bytes.length > maxSize * 1024 * 1024) {
      if (mounted) showMessage(context, forge.strings.fileTooLarge(maxSize));
      return;
    }
    setState(() => _uploading = true);
    try {
      final file = await forge.client
          .table(widget.field.table.name)
          .upload(
            _column.name,
            picked.name,
            bytes,
            contentType: contentTypeOf(picked.name),
          );
      widget.field.onChanged(file.toJson());
    } on ApiException catch (error) {
      if (!mounted) return;
      final message = error.fields[_column.name]?.join('\n');
      message == null
          ? showError(context, error)
          : showMessage(context, message);
    } finally {
      if (mounted) setState(() => _uploading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    final s = forge.strings;
    final file = ForgeFile.fromJson(widget.field.value);
    final label = forge.format.columnLabel(_column);
    final image = _column.type == ColumnType.image;
    return InputDecorator(
      decoration: InputDecoration(
        labelText: _column.required ? '$label *' : label,
        errorText: widget.field.error,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (file != null) ...[
            if (image) ImageValue(file, size: 120) else FileValue(file),
            const SizedBox(height: 8),
          ],
          if (_uploading) ...[
            const LinearProgressIndicator(),
            const SizedBox(height: 4),
            Text(s.uploading),
          ] else
            Wrap(
              spacing: 8,
              children: [
                FilledButton.tonalIcon(
                  onPressed: _choose,
                  icon: Icon(
                    image
                        ? Icons.add_photo_alternate_outlined
                        : Icons.upload_file,
                  ),
                  label: Text(
                    file != null
                        ? s.replace
                        : image
                        ? s.chooseImage
                        : s.chooseFile,
                  ),
                ),
                if (file != null && !_column.required)
                  TextButton.icon(
                    onPressed: () => widget.field.onChanged(null),
                    icon: const Icon(Icons.close),
                    label: Text(s.remove),
                  ),
              ],
            ),
        ],
      ),
    );
  }
}

// -------------------------------------------------------- autres champs

InputDecoration fieldDecoration(BuildContext context, FieldState field) {
  final label = Forge.of(context).format.columnLabel(field.column);
  return InputDecoration(
    labelText: field.column.required ? '$label *' : label,
    errorText: field.error,
  );
}

class ColorField extends StatelessWidget {
  const ColorField(this.field, {super.key});

  final FieldState field;

  @override
  Widget build(BuildContext context) {
    final hex = field.value is String ? field.value as String : null;
    return InkWell(
      onTap: () async {
        final picked = await pickColor(context, hex);
        if (picked != null) field.onChanged(picked);
      },
      child: InputDecorator(
        decoration: fieldDecoration(context, field).copyWith(
          suffixIcon: hex == null || field.column.required
              ? const Icon(Icons.palette_outlined)
              : IconButton(
                  tooltip: Forge.of(context).strings.clear,
                  icon: const Icon(Icons.clear),
                  onPressed: () => field.onChanged(null),
                ),
        ),
        isEmpty: hex == null,
        child: hex == null ? const Text('') : ColorValue(hex),
      ),
    );
  }
}

class RatingField extends StatelessWidget {
  const RatingField(this.field, {super.key});

  final FieldState field;

  @override
  Widget build(BuildContext context) {
    final value = field.value is int ? field.value as int : null;
    return InputDecorator(
      decoration: fieldDecoration(context, field).copyWith(
        suffixIcon: value == null || field.column.required
            ? null
            : IconButton(
                tooltip: Forge.of(context).strings.clear,
                icon: const Icon(Icons.clear),
                onPressed: () => field.onChanged(null),
              ),
      ),
      child: Align(
        alignment: AlignmentDirectional.centerStart,
        child: RatingStars(
          value: value ?? 0,
          max: field.column.max,
          size: 28,
          onChanged: field.onChanged,
        ),
      ),
    );
  }
}

/// Texte Markdown : saisie, et aperçu du rendu.
class MarkdownField extends StatefulWidget {
  const MarkdownField(this.field, {super.key});

  final FieldState field;

  @override
  State<MarkdownField> createState() => _MarkdownFieldState();
}

class _MarkdownFieldState extends State<MarkdownField> {
  late final _controller = TextEditingController(
    text: widget.field.value is String ? widget.field.value as String : '',
  );
  bool _preview = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = Forge.of(context).strings;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: SegmentedButton<bool>(
            showSelectedIcon: false,
            segments: [
              ButtonSegment(
                value: false,
                label: Text(s.write),
                icon: const Icon(Icons.edit_outlined),
              ),
              ButtonSegment(
                value: true,
                label: Text(s.preview),
                icon: const Icon(Icons.visibility_outlined),
              ),
            ],
            selected: {_preview},
            onSelectionChanged: (value) =>
                setState(() => _preview = value.first),
          ),
        ),
        const SizedBox(height: 8),
        if (_preview)
          InputDecorator(
            decoration: fieldDecoration(context, widget.field),
            child: MarkdownValue(_controller.text),
          )
        else
          TextField(
            controller: _controller,
            decoration: fieldDecoration(context, widget.field),
            keyboardType: TextInputType.multiline,
            minLines: 5,
            maxLines: 14,
            onChanged: (text) =>
                widget.field.onChanged(text.isEmpty ? null : text),
          ),
      ],
    );
  }
}

/// Formulaire de création de [table], empilé ; renvoie l'identifiant créé
/// (`null` si l'utilisateur renonce).
Future<int?> createRecordFor(BuildContext context, TableSchema table) =>
    context.push<int>(Paths.pick(table.name));

/// Création d'un enregistrement de [table] depuis un champ de référence :
/// formulaire empilé, qui revient avec l'identifiant créé.
class CreateReferenceButton extends StatelessWidget {
  const CreateReferenceButton({
    super.key,
    required this.table,
    required this.onCreated,
  });

  final TableSchema table;
  final ValueChanged<int> onCreated;

  @override
  Widget build(BuildContext context) {
    final forge = Forge.of(context);
    if (!table.allows(forge.roles, Operation.create)) {
      return const SizedBox.shrink();
    }
    return IconButton(
      tooltip: forge.strings.newRecord(forge.format.tableLabel(table)),
      icon: const Icon(Icons.add_circle_outline),
      onPressed: () async {
        final id = await createRecordFor(context, table);
        if (id != null) onCreated(id);
      },
    );
  }
}
