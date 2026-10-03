/// Fichier téléversé, tel que lu dans un enregistrement (colonnes `file` et
/// `image`). [url] est un lien temporaire, utilisable sans jeton.
class ForgeFile {
  const ForgeFile({
    required this.id,
    required this.name,
    required this.size,
    required this.contentType,
    required this.url,
  });

  static ForgeFile? fromJson(Object? json) => json is Map
      ? ForgeFile(
          id: json['id'] as String,
          name: json['name'] as String,
          size: json['size'] as int,
          contentType: json['content_type'] as String,
          url: json['url'] as String,
        )
      : null;

  final String id;
  final String name;

  /// Taille en octets.
  final int size;
  final String contentType;
  final String url;

  bool get isImage => contentType.startsWith('image/');

  Map<String, Object> toJson() => {
    'id': id,
    'name': name,
    'size': size,
    'content_type': contentType,
    'url': url,
  };
}
