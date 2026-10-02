//! Tables de jointure des colonnes `reference_list` (`source_id`, `target_id`).

use std::collections::BTreeMap;

use sea_orm::sea_query::{Alias, Expr, ExprTrait, Query};
use sea_orm::{ConnectionTrait, DbErr};

pub(crate) const SOURCE: &str = "source_id";
pub(crate) const TARGET: &str = "target_id";

/// Cibles liées à chacun des `ids` : `source_id → [target_id]` (triés).
pub(crate) async fn load(
    db: &impl ConnectionTrait,
    join_table: &str,
    ids: &[i64],
) -> Result<BTreeMap<i64, Vec<i64>>, DbErr> {
    let mut links: BTreeMap<i64, Vec<i64>> = ids.iter().map(|id| (*id, Vec::new())).collect();
    if ids.is_empty() {
        return Ok(links);
    }
    let select = Query::select()
        .columns([Alias::new(SOURCE), Alias::new(TARGET)])
        .from(Alias::new(join_table))
        .and_where(Expr::col(Alias::new(SOURCE)).is_in(ids.iter().copied()))
        .order_by(Alias::new(TARGET), sea_orm::Order::Asc)
        .to_owned();
    for row in db.query_all(&select).await? {
        let source: i64 = row.try_get("", SOURCE)?;
        let target: i64 = row.try_get("", TARGET)?;
        links.entry(source).or_default().push(target);
    }
    Ok(links)
}

/// Remplace les liens de `source` par `targets`.
pub(crate) async fn replace(
    db: &impl ConnectionTrait,
    join_table: &str,
    source: i64,
    targets: &[i64],
) -> Result<(), DbErr> {
    let delete = Query::delete()
        .from_table(Alias::new(join_table))
        .and_where(Expr::col(Alias::new(SOURCE)).eq(source))
        .to_owned();
    db.execute(&delete).await?;
    if targets.is_empty() {
        return Ok(());
    }
    let mut insert = Query::insert()
        .into_table(Alias::new(join_table))
        .columns([Alias::new(SOURCE), Alias::new(TARGET)])
        .to_owned();
    for target in targets {
        insert.values_panic([source.into(), (*target).into()]);
    }
    db.execute(&insert).await?;
    Ok(())
}
