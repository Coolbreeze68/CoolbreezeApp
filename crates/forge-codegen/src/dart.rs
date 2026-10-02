//! Écriture d'expressions Dart mises en forme comme le fait `dart format` :
//! sur une ligne si elle tient en 80 colonnes, sinon un élément par ligne avec
//! une virgule finale (conservée par `trailing_commas: preserve`).

use std::fmt::Write as _;

/// Largeur de page de `dart format`.
const WIDTH: usize = 80;

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    /// Code inséré tel quel.
    Raw(String),
    /// Chaîne littérale.
    Str(String),
    /// Appel (ou constructeur) : `callee(args)`.
    Call(String, Vec<Arg>),
    List(Vec<Expr>),
    Set(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
}

#[derive(Debug, Clone)]
pub(crate) struct Arg {
    pub name: Option<String>,
    pub value: Expr,
}

pub(crate) fn raw(code: impl Into<String>) -> Expr {
    Expr::Raw(code.into())
}

pub(crate) fn string(text: &str) -> Expr {
    Expr::Str(text.to_owned())
}

pub(crate) fn call(callee: impl Into<String>, args: Vec<Arg>) -> Expr {
    Expr::Call(callee.into(), args)
}

pub(crate) fn pos(value: Expr) -> Arg {
    Arg { name: None, value }
}

pub(crate) fn named(name: &str, value: Expr) -> Arg {
    Arg {
        name: Some(name.to_owned()),
        value,
    }
}

/// Littéral de chaîne entre apostrophes.
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn width(text: &str) -> usize {
    text.chars().count()
}

impl Expr {
    /// Forme sur une seule ligne.
    fn flat(&self) -> String {
        let join = |items: Vec<String>| items.join(", ");
        match self {
            Self::Raw(code) => code.clone(),
            Self::Str(text) => quote(text),
            Self::Call(callee, args) => {
                format!("{callee}({})", join(args.iter().map(Arg::flat).collect()))
            }
            Self::List(items) => format!("[{}]", join(items.iter().map(Self::flat).collect())),
            Self::Set(items) => format!("{{{}}}", join(items.iter().map(Self::flat).collect())),
            Self::Map(entries) => format!(
                "{{{}}}",
                join(
                    entries
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k.flat(), v.flat()))
                        .collect()
                )
            ),
        }
    }

    fn is_collection(&self) -> bool {
        matches!(self, Self::List(_) | Self::Set(_) | Self::Map(_))
    }

    /// Rend l'expression ; `indent` : indentation de la ligne où elle commence,
    /// `column` : colonne où elle commence, `suffix` : largeur de ce qui la suit
    /// sur la même ligne (`,` ou `;`).
    pub(crate) fn render(&self, indent: usize, column: usize, suffix: usize) -> String {
        let flat = self.flat();
        if column + width(&flat) + suffix <= WIDTH {
            return flat;
        }
        let inner = indent + 2;
        let pad = " ".repeat(inner);
        let close = " ".repeat(indent);
        let lines = |items: Vec<String>| -> String {
            items.into_iter().fold(String::new(), |mut out, item| {
                let _ = writeln!(out, "{pad}{item},");
                out
            })
        };
        match self {
            Self::Raw(_) | Self::Str(_) => flat,
            Self::Call(callee, args) => {
                // Un seul argument collection : collé aux parenthèses.
                if let [Arg { name: None, value }] = args.as_slice()
                    && value.is_collection()
                {
                    let start = column + width(callee) + 1;
                    return format!("{callee}({})", value.render(indent, start, suffix + 1));
                }
                format!(
                    "{callee}(\n{}{close})",
                    lines(args.iter().map(|a| a.render(inner)).collect())
                )
            }
            Self::List(items) | Self::Set(items) => {
                let (open, end) = if matches!(self, Self::List(_)) {
                    ('[', ']')
                } else {
                    ('{', '}')
                };
                let items = items
                    .iter()
                    .map(|item| item.render(inner, inner, 1))
                    .collect();
                format!("{open}\n{}{close}{end}", lines(items))
            }
            Self::Map(entries) => {
                let items = entries
                    .iter()
                    .map(|(key, value)| {
                        let key = key.flat();
                        let start = inner + width(&key) + 2;
                        format!("{key}: {}", value.render(inner, start, 1))
                    })
                    .collect();
                format!("{{\n{}{close}}}", lines(items))
            }
        }
    }
}

impl Arg {
    fn flat(&self) -> String {
        match &self.name {
            Some(name) => format!("{name}: {}", self.value.flat()),
            None => self.value.flat(),
        }
    }

    /// Argument seul sur sa ligne, suivi d'une virgule.
    fn render(&self, indent: usize) -> String {
        match &self.name {
            Some(name) => format!(
                "{name}: {}",
                self.value.render(indent, indent + width(name) + 2, 1)
            ),
            None => self.value.render(indent, indent, 1),
        }
    }
}

/// Déclaration `prefix expr;` (ou `,`) à l'indentation `indent`. Après `=>`,
/// une expression dont même le début ne tient pas passe à la ligne suivante.
pub(crate) fn statement(indent: usize, prefix: &str, expr: &Expr, end: &str) -> String {
    let pad = " ".repeat(indent);
    let column = indent + width(prefix);
    let fits = column + width(&expr.flat()) + width(end) <= WIDTH;
    let head = match expr {
        Expr::Call(callee, _) => width(callee) + 1,
        _ => 1,
    };
    if !fits && prefix.ends_with("=> ") && column + head > WIDTH {
        let inner = indent + 4;
        return format!(
            "{pad}{}\n{}{}{end}\n",
            prefix.trim_end(),
            " ".repeat(inner),
            expr.render(inner, inner, width(end))
        );
    }
    format!(
        "{pad}{prefix}{}{end}\n",
        expr.render(indent, column, width(end))
    )
}

/// Identifiant `lowerCamelCase` d'un nom `snake_case`.
pub(crate) fn camel_case(name: &str) -> String {
    let pascal = forge_schema::names::pascal_case(name);
    let mut chars = pascal.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + chars.as_str()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_expressions_stay_on_one_line() {
        let expr = call(
            "Label",
            vec![pos(Expr::Map(vec![(string("fr"), string("Nom"))]))],
        );
        assert_eq!(expr.render(0, 0, 0), "Label({'fr': 'Nom'})");
    }

    #[test]
    fn long_calls_split_one_argument_per_line() {
        let expr = call(
            "ColumnSchema",
            vec![
                pos(string("une_colonne_au_nom_long")),
                pos(raw("ColumnType.string")),
                named("required", raw("true")),
                named("titleField", raw("true")),
            ],
        );
        assert_eq!(
            statement(2, "", &expr, ","),
            "  ColumnSchema(\n    'une_colonne_au_nom_long',\n    ColumnType.string,\n    \
             required: true,\n    titleField: true,\n  ),\n"
        );
    }

    #[test]
    fn single_collection_argument_is_hugged() {
        let expr = call(
            "Label",
            vec![pos(Expr::Map(vec![
                (string("fr"), string("Un libellé assez long pour déborder")),
                (string("en"), string("A label long enough to overflow")),
            ]))],
        );
        assert_eq!(
            statement(4, "label: ", &expr, ","),
            "    label: Label({\n      'fr': 'Un libellé assez long pour déborder',\n      \
             'en': 'A label long enough to overflow',\n    }),\n"
        );
    }

    #[test]
    fn arrow_body_moves_to_next_line_when_its_start_overflows() {
        let expr = call("Client", vec![pos(raw("a_rather_long_argument_name"))]);
        let prefix =
            "static TableClient<UnNomDeClasseVraimentTresLong> api(ForgeClient client) => ";
        assert_eq!(
            statement(2, prefix, &expr, ";"),
            "  static TableClient<UnNomDeClasseVraimentTresLong> api(ForgeClient client) =>\n      \
             Client(a_rather_long_argument_name);\n"
        );
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(quote("l'été à 5$\\"), r"'l\'été à 5\$\\'");
        assert_eq!(camel_case("date_cloture"), "dateCloture");
    }
}
