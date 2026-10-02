use crate::ast::Span;

/// Erreur de syntaxe, localisée dans le texte source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} (position {})", span.start)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}
