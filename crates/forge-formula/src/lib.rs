//! Langage de formules de forge.
//!
//! Utilisé pour les colonnes calculées (`formula`) et pour les conditions
//! des règles d'autorisation (`when`) : analyse ([`parse`]), vérification des
//! types ([`typecheck`]) et évaluation ([`evaluate`]).
//!
//! ```
//! use forge_formula::{parse, ExprKind};
//!
//! let expr = parse("montant * probabilite / 100").unwrap();
//! assert!(matches!(expr.kind, ExprKind::Binary { .. }));
//! ```

mod ast;
mod error;
mod eval;
mod functions;
mod lexer;
mod parser;
mod typecheck;
mod value;

pub use ast::{BinaryOp, Expr, ExprKind, Span, UnaryOp, VarScope};
pub use error::ParseError;
pub use eval::{Env, EvalError, evaluate};
pub use functions::{
    Arity, FunctionKind, FunctionRegistry, FunctionSignature, Implementation, Returns,
};
pub use lexer::KEYWORDS;
pub use parser::parse;
pub use typecheck::{TypeEnv, TypeError, typecheck};
pub use value::{Type, Value};
