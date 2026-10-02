//! Hachage des mots de passe (argon2id, paramètres par défaut recommandés).

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

use crate::error::Error;

pub(crate) const MIN_LENGTH: usize = 8;

pub(crate) fn hash(password: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| Error::Config(format!("hachage du mot de passe : {err}")))
}

/// Vérifie un mot de passe ; un hash absent ou illisible ne correspond à rien.
pub(crate) fn verify(password: &str, hash: Option<&str>) -> bool {
    let Some(parsed) = hash.and_then(|h| PasswordHash::new(h).ok()) else {
        // Calcul équivalent, pour ne pas révéler par le temps de réponse
        // si le compte existe.
        let _ = self::hash(password);
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub(crate) fn check_strength(password: &str) -> Result<(), Error> {
    if password.chars().count() < MIN_LENGTH {
        return Err(Error::validation(
            "password",
            format!("{MIN_LENGTH} caractères au minimum"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let hash = hash("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify("correct horse", Some(&hash)));
        assert!(!verify("wrong horse", Some(&hash)));
        assert!(!verify("correct horse", None));
        assert!(!verify("correct horse", Some("pas un hash")));
    }

    #[test]
    fn strength() {
        assert!(check_strength("1234567").is_err());
        assert!(check_strength("12345678").is_ok());
    }
}
