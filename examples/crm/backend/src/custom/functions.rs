//! Fonctions de formule personnalisées. Ce fichier vous appartient : forge ne le modifie jamais.
//!
//! Déclarez chaque fonction dans `forge.json` (`functions` : nom, types des
//! arguments et du résultat), puis implémentez-la ici. L'application refuse de
//! démarrer si une fonction déclarée n'est pas implémentée.
//!
//! ```ignore
//! use forge_runtime::formula::{Decimal, Value};
//!
//! pub fn register(app: App) -> App {
//!     app.function("REMISE", |args| match args {
//!         [Value::Number(montant), Value::Number(taux)] => {
//!             Ok(Value::Number(*montant * (Decimal::ONE - *taux / Decimal::ONE_HUNDRED)))
//!         }
//!         _ => Ok(Value::Null),
//!     })
//! }
//! ```

use forge_runtime::App;

/// Ajoute à l'application les implémentations des fonctions déclarées.
pub fn register(app: App) -> App {
    app
}
