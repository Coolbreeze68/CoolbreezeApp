//! Parser à descente récursive.
//!
//! Précédence, de la plus faible à la plus forte :
//! `OR` < `AND` < `NOT` < comparaisons < `+ -` < `* /` < `-` unaire < primaire.
//! Les comparaisons ne sont pas associatives : `a < b < c` est refusé.

use crate::ast::{BinaryOp, Expr, ExprKind, Span, UnaryOp, VarScope};
use crate::error::ParseError;
use crate::lexer::{Token, tokenize};

/// Analyse une formule complète.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let mut parser = Parser {
        tokens: tokenize(src)?,
        pos: 0,
    };
    if parser.peek() == &Token::Eof {
        return Err(ParseError::new("formule vide", parser.span()));
    }
    let expr = parser.or()?;
    match parser.peek() {
        Token::Eof => Ok(expr),
        other => Err(ParseError::new(
            format!("{} inattendu", other.describe()),
            parser.span(),
        )),
    }
}

struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].1
    }

    fn advance(&mut self) -> (Token, Span) {
        let current = self.tokens[self.pos].clone();
        if current.0 != Token::Eof {
            self.pos += 1;
        }
        current
    }

    fn eat(&mut self, token: &Token) -> bool {
        let found = self.peek() == token;
        if found {
            self.advance();
        }
        found
    }

    fn expect(&mut self, token: &Token, what: &str) -> Result<Span, ParseError> {
        if self.peek() == token {
            Ok(self.advance().1)
        } else {
            Err(ParseError::new(
                format!("{what} attendu, trouvé {}", self.peek().describe()),
                self.span(),
            ))
        }
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.and()?;
        while self.eat(&Token::Or) {
            left = binary(BinaryOp::Or, left, self.and()?);
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.not()?;
        while self.eat(&Token::And) {
            left = binary(BinaryOp::And, left, self.not()?);
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Expr, ParseError> {
        if self.peek() == &Token::Not {
            let (_, start) = self.advance();
            let expr = self.not()?;
            return Ok(unary(UnaryOp::Not, start, expr));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.additive()?;
        let Some(op) = comparison_op(self.peek()) else {
            return Ok(left);
        };
        self.advance();
        let expr = binary(op, left, self.additive()?);
        if comparison_op(self.peek()).is_some() {
            return Err(ParseError::new(
                "comparaisons enchaînées interdites : utilisez AND",
                self.span(),
            ));
        }
        Ok(expr)
    }

    fn additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => return Ok(left),
            };
            self.advance();
            left = binary(op, left, self.multiplicative()?);
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                _ => return Ok(left),
            };
            self.advance();
            left = binary(op, left, self.unary()?);
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.peek() == &Token::Minus {
            let (_, start) = self.advance();
            let expr = self.unary()?;
            return Ok(unary(UnaryOp::Neg, start, expr));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let (token, span) = self.advance();
        let kind = match token {
            Token::Number(n) => ExprKind::Number(n),
            Token::String(s) => ExprKind::String(s),
            Token::True => ExprKind::Bool(true),
            Token::False => ExprKind::Bool(false),
            Token::Null => ExprKind::Null,
            Token::LParen => {
                let inner = self.or()?;
                let end = self.expect(&Token::RParen, "`)`")?;
                return Ok(Expr {
                    kind: inner.kind,
                    span: span.to(end),
                });
            }
            Token::Var(name) => return self.var(&name, span),
            Token::Ident(name) if self.peek() == &Token::LParen => return self.call(&name, span),
            Token::Ident(name) => return Ok(self.path(name, span)),
            other => {
                return Err(ParseError::new(
                    format!("valeur attendue, trouvé {}", other.describe()),
                    span,
                ));
            }
        };
        Ok(Expr { kind, span })
    }

    /// Lit la suite `.segment` d'un chemin déjà commencé.
    fn path_tail(&mut self, mut path: Vec<String>, mut span: Span) -> (Vec<String>, Span) {
        while self.peek() == &Token::Dot {
            let Token::Ident(_) = &self.tokens[self.pos + 1].0 else {
                break;
            };
            self.advance();
            let (Token::Ident(segment), end) = self.advance() else {
                unreachable!("vérifié ci-dessus");
            };
            path.push(segment);
            span = span.to(end);
        }
        (path, span)
    }

    fn path(&mut self, first: String, span: Span) -> Expr {
        let (path, span) = self.path_tail(vec![first], span);
        Expr {
            kind: ExprKind::Path(path),
            span,
        }
    }

    fn var(&mut self, name: &str, span: Span) -> Result<Expr, ParseError> {
        let scope = VarScope::from_name(name).ok_or_else(|| {
            ParseError::new(
                format!("variable inconnue `${name}` (attendu `$user` ou `$param`)"),
                span,
            )
        })?;
        let (path, span) = self.path_tail(Vec::new(), span);
        if path.is_empty() {
            return Err(ParseError::new(
                format!("champ attendu après `${name}`, par exemple `${name}.id`"),
                span,
            ));
        }
        Ok(Expr {
            kind: ExprKind::Var { scope, path },
            span,
        })
    }

    fn call(&mut self, name: &str, start: Span) -> Result<Expr, ParseError> {
        self.expect(&Token::LParen, "`(`")?;
        let mut args = Vec::new();
        if self.peek() != &Token::RParen {
            loop {
                args.push(self.or()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        let end = self.expect(&Token::RParen, "`)` ou `,`")?;
        Ok(Expr {
            kind: ExprKind::Call {
                name: name.to_ascii_uppercase(),
                args,
            },
            span: start.to(end),
        })
    }
}

fn comparison_op(token: &Token) -> Option<BinaryOp> {
    Some(match token {
        Token::Eq => BinaryOp::Eq,
        Token::Ne => BinaryOp::Ne,
        Token::Lt => BinaryOp::Lt,
        Token::Le => BinaryOp::Le,
        Token::Gt => BinaryOp::Gt,
        Token::Ge => BinaryOp::Ge,
        _ => return None,
    })
}

fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr {
        span: left.span.to(right.span),
        kind: ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}

fn unary(op: UnaryOp, start: Span, expr: Expr) -> Expr {
    Expr {
        span: start.to(expr.span),
        kind: ExprKind::Unary {
            op,
            expr: Box::new(expr),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Représentation compacte et parenthésée de l'AST, pour des assertions lisibles.
    fn sexpr(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Number(n) => n.to_string(),
            ExprKind::String(s) => format!("{s:?}"),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Null => "null".into(),
            ExprKind::Path(p) => p.join("."),
            ExprKind::Var { scope, path } => format!("${}.{}", scope.name(), path.join(".")),
            ExprKind::Unary { op, expr } => format!("({op:?} {})", sexpr(expr)),
            ExprKind::Binary { op, left, right } => {
                format!("({op:?} {} {})", sexpr(left), sexpr(right))
            }
            ExprKind::Call { name, args } => {
                let args: Vec<_> = args.iter().map(sexpr).collect();
                format!("{name}({})", args.join(" "))
            }
        }
    }

    fn check(src: &str, expected: &str) {
        assert_eq!(sexpr(&parse(src).unwrap()), expected, "source : {src}");
    }

    #[test]
    fn arithmetic_precedence() {
        check(
            "montant * probabilite / 100",
            "(Div (Mul montant probabilite) 100)",
        );
        check("1 + 2 * 3", "(Add 1 (Mul 2 3))");
        check("(1 + 2) * 3", "(Mul (Add 1 2) 3)");
        check("-a - -b", "(Sub (Neg a) (Neg b))");
    }

    #[test]
    fn boolean_precedence() {
        check(
            "a == 1 OR b > 2 AND NOT c",
            "(Or (Eq a 1) (And (Gt b 2) (Not c)))",
        );
        check("owner == $user.id", "(Eq owner $user.id)");
    }

    #[test]
    fn paths_vars_and_calls() {
        check("entreprise.secteur", "entreprise.secteur");
        check("$param.tva", "$param.tva");
        check(
            "ROUND(montant * (1 + $param.tva / 100), 2)",
            "ROUND((Mul montant (Add 1 (Div $param.tva 100))) 2)",
        );
        check("sum(opportunites.montant)", "SUM(opportunites.montant)");
        check("TODAY()", "TODAY()");
        check(r#"IF(x, "oui", NULL)"#, r#"IF(x "oui" null)"#);
    }

    #[test]
    fn spans_cover_whole_expression() {
        let expr = parse(" a + f(b) ").unwrap();
        assert_eq!(expr.span, Span::new(1, 9));
    }

    #[test]
    fn syntax_errors_are_located() {
        let cases = [
            ("", 0),
            ("a +", 3),
            ("a b", 2),
            ("f(a", 3),
            ("(a", 2),
            ("a < b < c", 6),
            ("$foo.x", 0),
            ("$user", 0),
            ("a.", 1),
        ];
        for (src, position) in cases {
            let err = parse(src).unwrap_err();
            assert_eq!(err.span.start, position, "source : {src:?}, erreur : {err}");
        }
    }
}
