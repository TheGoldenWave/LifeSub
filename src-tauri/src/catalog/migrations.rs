use std::time::Duration;

use rusqlite::{Connection, Error, TransactionBehavior, ffi};

use self::ddl::{ASR_SCHEMA, CURRENT_VERSION, FRESH_BASE_SCHEMA, LEGACY_ALTERS};
use self::fingerprint::{classify_locked, is_v2};

mod ddl;
mod fingerprint;

const MIGRATION_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaKind {
    Fresh,
    LegacyV1,
    CurrentV2,
    Unknown,
}

#[cfg(test)]
pub(crate) fn classify(connection: &mut Connection) -> rusqlite::Result<SchemaKind> {
    configure_connection(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let kind = classify_locked(&transaction)?;
    transaction.commit()?;
    Ok(kind)
}

pub(crate) fn migrate(connection: &mut Connection) -> rusqlite::Result<()> {
    migrate_with_hooks(connection, || Ok(()), || Ok(()))
}

#[cfg(test)]
pub(crate) fn migrate_with_hook<F>(connection: &mut Connection, hook: F) -> rusqlite::Result<()>
where
    F: FnOnce() -> rusqlite::Result<()>,
{
    migrate_with_hooks(connection, || Ok(()), hook)
}

#[cfg(test)]
pub(crate) fn migrate_with_classification_hook<F>(
    connection: &mut Connection,
    hook: F,
) -> rusqlite::Result<()>
where
    F: FnOnce() -> rusqlite::Result<()>,
{
    migrate_with_hooks(connection, hook, || Ok(()))
}

fn migrate_with_hooks<C, D>(
    connection: &mut Connection,
    classification_hook: C,
    ddl_hook: D,
) -> rusqlite::Result<()>
where
    C: FnOnce() -> rusqlite::Result<()>,
    D: FnOnce() -> rusqlite::Result<()>,
{
    configure_connection(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let kind = classify_locked(&transaction)?;
    classification_hook()?;
    match kind {
        SchemaKind::CurrentV2 => return transaction.commit(),
        SchemaKind::Unknown => return Err(migration_error("unknown or corrupt catalog schema")),
        SchemaKind::Fresh | SchemaKind::LegacyV1 => {}
    }
    match kind {
        SchemaKind::Fresh => transaction.execute_batch(FRESH_BASE_SCHEMA)?,
        SchemaKind::LegacyV1 => transaction.execute_batch(LEGACY_ALTERS)?,
        SchemaKind::CurrentV2 | SchemaKind::Unknown => unreachable!(),
    }
    transaction.execute_batch(ASR_SCHEMA)?;
    ddl_hook()?;
    if !is_v2(&transaction)? {
        return Err(migration_error(
            "migration produced invalid v2 catalog schema",
        ));
    }
    transaction.pragma_update(None, "user_version", CURRENT_VERSION)?;
    transaction.commit()
}

fn configure_connection(connection: &Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(MIGRATION_BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)
}

#[cfg(test)]
pub(crate) fn normalize_sql_for_test(sql: &str) -> String {
    fingerprint::normalize_sql(sql)
}

fn migration_error(message: &str) -> Error {
    Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_SCHEMA),
        Some(message.to_owned()),
    )
}
