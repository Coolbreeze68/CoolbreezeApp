//! Points d'extension par table, implémentés dans `src/custom/hooks/<table>.rs`.

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Mutex, PoisonError};

use sea_orm::{DatabaseTransaction, EntityTrait};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::Error;

/// Contexte transmis aux hooks. Toutes les opérations d'une requête
/// (hooks compris) s'exécutent dans la même transaction.
#[derive(Debug)]
pub struct HookContext<'a> {
    pub(crate) txn: &'a DatabaseTransaction,
    pub(crate) model: &'a forge_schema::Model,
    pub(crate) user: &'a CurrentUser,
    pub(crate) written: &'a Written,
}

impl<'a> HookContext<'a> {
    pub(crate) fn new(
        txn: &'a DatabaseTransaction,
        state: &'a AppState,
        user: &'a CurrentUser,
        written: &'a Written,
    ) -> Self {
        Self {
            txn,
            model: &state.model,
            user,
            written,
        }
    }

    /// Transaction en cours : à utiliser pour toute lecture ou écriture.
    pub fn db(&self) -> &DatabaseTransaction {
        self.txn
    }

    /// Schéma de l'application.
    pub fn schema(&self) -> &forge_schema::Model {
        self.model
    }

    /// Utilisateur à l'origine de la requête.
    pub fn user(&self) -> &CurrentUser {
        self.user
    }

    /// Signale une table modifiée par le hook (`ctx.db()`), pour que les lectures
    /// en cache qui en dépendent soient invalidées une fois la transaction validée.
    pub fn modified(&self, table: &str) {
        self.written.add(table);
    }
}

/// Effets d'une écriture à appliquer une fois la transaction validée : tables
/// à invalider dans le cache, fichiers à retirer du stockage.
#[derive(Debug, Default)]
pub(crate) struct Written {
    tables: Mutex<BTreeSet<String>>,
    files: Mutex<Vec<String>>,
}

impl Written {
    pub(crate) fn add(&self, table: &str) {
        lock(&self.tables).insert(table.to_owned());
    }

    pub(crate) fn extend(&self, tables: BTreeSet<String>) {
        lock(&self.tables).extend(tables);
    }

    /// Fichier détaché d'un enregistrement (remplacé ou supprimé avec lui).
    pub(crate) fn remove_file(&self, id: String) {
        lock(&self.files).push(id);
    }

    pub(crate) fn into_parts(self) -> (BTreeSet<String>, Vec<String>) {
        let tables = self
            .tables
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        let files = self
            .files
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        (tables, files)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Hooks d'une table. Chaque méthode a une implémentation par défaut qui ne fait
/// rien : surchargez uniquement celles dont vous avez besoin.
///
/// Ordre d'appel :
/// - création : `before_create` → `validate` → insertion → `after_create` ;
/// - modification : `before_update` → `validate` → mise à jour → `after_update` ;
/// - suppression : `before_delete` → suppression.
///
/// Une erreur annule toute l'opération (transaction). Pour signaler un champ
/// invalide : `Err(Error::validation("montant", "doit être positif"))`.
pub trait Hooks<E: EntityTrait>: Send + Sync + 'static {
    fn validate(
        &self,
        ctx: &HookContext<'_>,
        record: &E::ActiveModel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }

    fn before_create(
        &self,
        ctx: &HookContext<'_>,
        record: &mut E::ActiveModel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }

    fn after_create(
        &self,
        ctx: &HookContext<'_>,
        record: &E::Model,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }

    fn before_update(
        &self,
        ctx: &HookContext<'_>,
        record: &mut E::ActiveModel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }

    fn after_update(
        &self,
        ctx: &HookContext<'_>,
        record: &E::Model,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }

    fn before_delete(
        &self,
        ctx: &HookContext<'_>,
        record: &E::Model,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (ctx, record);
        async { Ok(()) }
    }
}
