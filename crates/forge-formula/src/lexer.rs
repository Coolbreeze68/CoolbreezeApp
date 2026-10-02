use rust_decimal::Decimal;

use crate::ast::Span;
use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Number(Decimal),
    String(String),
    Ident(String),
    /// `$nom` ; le nom est stocké sans le `$`.
    Var(String),
    True,
    False,
    Null,
    And,
    Or,
    Not,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    Comma,
    Dot,
    Eof,
}

impl Token {
    /// Libellé utilisé dans les messages d'erreur.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Number(n) => format!("nombre `{n}`"),
            Self::String(s) => format!("texte \"{s}\""),
            Self::Ident(name) => format!("`{name}`"),
            Self::Var(name) => format!("`${name}`"),
            Self::Eof => "fin de formule".to_owned(),
            other => format!("`{}`", other.symbol()),
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            Self::True => "TRUE",
            Self::False => "FALSE",
            Self::Null => "NULL",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Not => "NOT",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Star => "*",
            Self::Slash => "/",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::LParen => "(",
            Self::RParen => ")",
            Self::Comma => ",",
            Self::Dot => ".",
            Self::Number(_) | Self::String(_) | Self::Ident(_) | Self::Var(_) | Self::Eof => "",
        }
    }
}

/// Mots réservés du langage (insensibles à la casse).
pub const KEYWORDS: [&str; 6] = ["and", "or", "not", "true", "false", "null"];

pub(crate) fn tokenize(src: &str) -> Result<Vec<(Token, Span)>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = src.char_indices().peekable();

    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        let token = match c {
            '0'..='9' => lex_number(src, &mut chars)?,
            '"' => lex_string(src, &mut chars)?,
            '$' => {
                chars.next();
                let name = take_ident(src, &mut chars);
                if name.is_empty() {
                    return Err(ParseError::new(
                        "nom de variable attendu après `$`",
                        Span::new(start, start + 1),
                    ));
                }
                Token::Var(name.to_owned())
            }
            c if is_ident_start(c) => keyword_or_ident(take_ident(src, &mut chars)),
            _ => {
                chars.next();
                lex_symbol(c, &mut chars).ok_or_else(|| {
                    ParseError::new(
                        format!("caractère inattendu `{c}`"),
                        Span::new(start, start + c.len_utf8()),
                    )
                })?
            }
        };

        let end = chars.peek().map_or(src.len(), |&(i, _)| i);
        tokens.push((token, Span::new(start, end)));
    }

    tokens.push((Token::Eof, Span::new(src.len(), src.len())));
    Ok(tokens)
}

type Chars<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn take_ident<'a>(src: &'a str, chars: &mut Chars<'_>) -> &'a str {
    let start = chars.peek().map_or(src.len(), |&(i, _)| i);
    while chars
        .next_if(|&(_, c)| c.is_ascii_alphanumeric() || c == '_')
        .is_some()
    {}
    let end = chars.peek().map_or(src.len(), |&(i, _)| i);
    &src[start..end]
}

fn keyword_or_ident(word: &str) -> Token {
    match word.to_ascii_lowercase().as_str() {
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        "true" => Token::True,
        "false" => Token::False,
        "null" => Token::Null,
        _ => Token::Ident(word.to_owned()),
    }
}

fn lex_number(src: &str, chars: &mut Chars<'_>) -> Result<Token, ParseError> {
    let start = chars.peek().map_or(src.len(), |&(i, _)| i);
    while chars.next_if(|&(_, c)| c.is_ascii_digit()).is_some() {}

    // Partie décimale seulement si un chiffre suit le point : `a.b` reste un chemin.
    let mut lookahead = chars.clone();
    if lookahead.next().is_some_and(|(_, c)| c == '.')
        && lookahead.next().is_some_and(|(_, c)| c.is_ascii_digit())
    {
        chars.next();
        while chars.next_if(|&(_, c)| c.is_ascii_digit()).is_some() {}
    }

    let end = chars.peek().map_or(src.len(), |&(i, _)| i);
    let text = &src[start..end];
    text.parse::<rust_decimal::Decimal>()
        .map(Token::Number)
        .map_err(|_| ParseError::new(format!("nombre invalide `{text}`"), Span::new(start, end)))
}

fn lex_string(src: &str, chars: &mut Chars<'_>) -> Result<Token, ParseError> {
    let (start, _) = chars.next().expect("guillemet ouvrant");
    let mut value = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok(Token::String(value)),
            '\\' => match chars.next() {
                Some((_, '"')) => value.push('"'),
                Some((_, '\\')) => value.push('\\'),
                Some((_, 'n')) => value.push('\n'),
                Some((_, other)) => {
                    return Err(ParseError::new(
                        format!("échappement inconnu `\\{other}`"),
                        Span::new(i, i + 1 + other.len_utf8()),
                    ));
                }
                None => break,
            },
            c => value.push(c),
        }
    }
    Err(ParseError::new(
        "texte non terminé : `\"` manquant",
        Span::new(start, src.len()),
    ))
}

fn lex_symbol(c: char, chars: &mut Chars<'_>) -> Option<Token> {
    let mut followed_by = |next: char| chars.next_if(|&(_, c)| c == next).is_some();
    Some(match c {
        '+' => Token::Plus,
        '-' => Token::Minus,
        '*' => Token::Star,
        '/' => Token::Slash,
        '(' => Token::LParen,
        ')' => Token::RParen,
        ',' => Token::Comma,
        '.' => Token::Dot,
        // `=` est accepté comme alias de `==` (habitude des tableurs).
        '=' => {
            followed_by('=');
            Token::Eq
        }
        '!' if followed_by('=') => Token::Ne,
        '<' if followed_by('=') => Token::Le,
        '<' if followed_by('>') => Token::Ne,
        '<' => Token::Lt,
        '>' if followed_by('=') => Token::Ge,
        '>' => Token::Gt,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Token> {
        tokenize(src).unwrap().into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn distinguishes_decimal_from_path() {
        assert_eq!(
            kinds("1.5 a.b"),
            vec![
                Token::Number("1.5".parse().unwrap()),
                Token::Ident("a".into()),
                Token::Dot,
                Token::Ident("b".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn operators_and_aliases() {
        assert_eq!(
            kinds("== = != <> <= >= < >"),
            vec![
                Token::Eq,
                Token::Eq,
                Token::Ne,
                Token::Ne,
                Token::Le,
                Token::Ge,
                Token::Lt,
                Token::Gt,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn keywords_are_case_insensitive() {
        assert_eq!(
            kinds("and Or NOT true"),
            vec![Token::And, Token::Or, Token::Not, Token::True, Token::Eof]
        );
    }

    #[test]
    fn string_escapes() {
        assert_eq!(
            kinds(r#""a\"b\\c""#),
            vec![Token::String("a\"b\\c".into()), Token::Eof]
        );
    }

    #[test]
    fn spans_point_to_source() {
        let tokens = tokenize("  $user.id").unwrap();
        assert_eq!(tokens[0], (Token::Var("user".into()), Span::new(2, 7)));
    }

    #[test]
    fn errors() {
        assert_eq!(tokenize("a # b").unwrap_err().span, Span::new(2, 3));
        assert!(tokenize("\"abc").is_err());
        assert!(tokenize("$ x").is_err());
        assert!(tokenize("!").is_err());
    }
}
