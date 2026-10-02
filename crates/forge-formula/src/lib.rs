//! Langage de formules de forge.
//!
//! Utilisé pour les colonnes calculées (`formula`) et pour les conditions
//! des règles d'autorisation (`when`).
//!
//! ```
//! use forge_formula::{parse, ExprKind};
//!
//! let expr = parse("montant * probabilite / 100").unwrap();
//! assert!(matches!(expr.kind, ExprKind::Binary { .. }));
//! ```

mod ast;
mod error;
mod functions;
mod lexer;
mod parser;

pub use ast::{BinaryOp, Expr, ExprKind, Span, UnaryOp, VarScope};
pub use error::ParseError;
pub use functions::{Arity, FunctionKind, FunctionRegistry, FunctionSignature};
pub use lexer::KEYWORDS;
pub use parser::parse;
