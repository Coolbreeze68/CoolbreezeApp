//! Règles de nommage des identifiants (tables, colonnes, rôles…).
//!
//! Les noms deviennent des identifiants SQL, Rust et Dart : ils doivent être
//! valides et non réservés dans les trois langages.

/// Colonnes ajoutées automatiquement à chaque table.
pub const SYSTEM_COLUMNS: [&str; 4] = ["id", "created_at", "updated_at", "owner"];

/// Tables gérées par forge.
pub const SYSTEM_TABLES: [&str; 5] = [
    "users",
    "roles",
    "user_roles",
    "parameters",
    "refresh_tokens",
];

/// Champs de `$user` utilisables dans les conditions de règles.
pub const USER_FIELDS: [&str; 2] = ["id", "email"];

/// Segments réservés sous `/api/` : aucune table ne peut porter ces noms.
pub const RESERVED_ROUTES: [&str; 2] = ["auth", "graphql"];

/// Longueur maximale d'un identifiant (limite de Postgres).
const MAX_LEN: usize = 63;

/// Mots-clés réservés de Rust et de Dart (un nom devient champ ou module dans les deux).
const RESERVED: &[&str] = &[
    // Rust
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
    // Dart (mots strictement réservés, hors doublons)
    "assert", "case", "catch", "class", "default", "extends", "finally", "is", "new", "null",
    "rethrow", "switch", "this", "throw", "var", "void", "with",
];

/// Vérifie qu'un nom est un identifiant `snake_case` utilisable partout.
pub fn check_identifier(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let well_formed = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.ends_with('_')
        && !name.contains("__");
    if !well_formed {
        return Err(format!(
            "`{name}` n'est pas un identifiant valide : lettres minuscules, chiffres et `_`, \
             en commençant par une lettre"
        ));
    }
    if name.len() > MAX_LEN {
        return Err(format!("`{name}` dépasse {MAX_LEN} caractères"));
    }
    if RESERVED.contains(&name) || forge_formula::KEYWORDS.contains(&name) {
        return Err(format!("`{name}` est un mot réservé"));
    }
    Ok(())
}

/// `date_cloture` → `DateCloture` (noms de types Rust, Dart et GraphQL).
pub fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

/// Code de langue : `fr`, `en`, `pt_BR`…
pub fn check_locale(code: &str) -> Result<(), String> {
    let (lang, region) = code.split_once('_').unwrap_or((code, ""));
    let valid = lang.len() == 2
        && lang.chars().all(|c| c.is_ascii_lowercase())
        && (region.is_empty()
            || (region.len() == 2 && region.chars().all(|c| c.is_ascii_uppercase())));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "`{code}` n'est pas un code de langue valide (ex. `fr`, `pt_BR`)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal() {
        assert_eq!(pascal_case("date_cloture"), "DateCloture");
        assert_eq!(pascal_case("tag"), "Tag");
        assert_eq!(pascal_case("x2_y"), "X2Y");
    }

    #[test]
    fn identifiers() {
        for ok in ["client", "date_cloture", "x2"] {
            assert!(check_identifier(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "Client", "2x", "a-b", "a__b", "fin_", "_a", "type", "class", "and",
        ] {
            assert!(check_identifier(bad).is_err(), "{bad}");
        }
        assert!(check_identifier(&"a".repeat(64)).is_err());
    }

    #[test]
    fn locales() {
        for ok in ["fr", "en", "pt_BR"] {
            assert!(check_locale(ok).is_ok(), "{ok}");
        }
        for bad in ["FR", "fra", "pt-BR", "pt_br", ""] {
            assert!(check_locale(bad).is_err(), "{bad}");
        }
    }
}
