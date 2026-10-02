use rust_decimal::Decimal;

/// Intervalle `[start, end)` en octets dans le texte source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Plus petit intervalle couvrant `self` et `other`.
    #[must_use]
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(Decimal),
    String(String),
    Bool(bool),
    Null,
    /// Référence de colonne ou chemin de relations : `montant`, `entreprise.secteur`.
    Path(Vec<String>),
    /// Variable de contexte : `$user.id`, `$param.tva`.
    Var {
        scope: VarScope,
        path: Vec<String>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Appel de fonction ; `name` est normalisé en majuscules.
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarScope {
    /// Utilisateur courant (`$user`), disponible dans les règles.
    User,
    /// Table des paramètres (`$param`).
    Param,
}

impl VarScope {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "user" => Some(Self::User),
            "param" => Some(Self::Param),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Param => "param",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}
