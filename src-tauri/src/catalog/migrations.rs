use std::time::Duration;

use rusqlite::{Connection, Error, TransactionBehavior, ffi};

use self::ddl::{
    ASR_SCHEMA, CURRENT_VERSION, FRESH_BASE_SCHEMA, LEGACY_ALTERS, MODEL_MANAGER_V3_SCHEMA,
    TOOL_API_V4_SCHEMA, V5_SCHEMA,
};
use self::fingerprint::{classify_locked, is_v2, is_v3, is_v4, is_v5};

mod ddl;
mod fingerprint;

const MIGRATION_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaKind {
    Fresh,
    LegacyV1,
    CurrentV2,
    CurrentV3,
    CurrentV4,
    CurrentV5,
    /// A newer schema that still contains the complete v5 contract.
    CompatibleNewer,
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
        SchemaKind::CurrentV5 | SchemaKind::CompatibleNewer => return transaction.commit(),
        SchemaKind::Unknown => return Err(migration_error("unknown or corrupt catalog schema")),
        SchemaKind::Fresh
        | SchemaKind::LegacyV1
        | SchemaKind::CurrentV2
        | SchemaKind::CurrentV3
        | SchemaKind::CurrentV4 => {}
    }
    match kind {
        SchemaKind::Fresh => transaction.execute_batch(FRESH_BASE_SCHEMA)?,
        SchemaKind::LegacyV1 => transaction.execute_batch(LEGACY_ALTERS)?,
        SchemaKind::CurrentV2 | SchemaKind::CurrentV3 | SchemaKind::CurrentV4 => {}
        SchemaKind::CurrentV5 | SchemaKind::CompatibleNewer | SchemaKind::Unknown => unreachable!(),
    }
    if matches!(kind, SchemaKind::Fresh | SchemaKind::LegacyV1) {
        transaction.execute_batch(ASR_SCHEMA)?;
    }
    if matches!(
        kind,
        SchemaKind::Fresh | SchemaKind::LegacyV1 | SchemaKind::CurrentV2
    ) && !is_v2(&transaction)?
    {
        return Err(migration_error(
            "migration produced invalid v2 catalog schema",
        ));
    }
    if matches!(
        kind,
        SchemaKind::Fresh | SchemaKind::LegacyV1 | SchemaKind::CurrentV2
    ) {
        transaction.execute_batch(MODEL_MANAGER_V3_SCHEMA)?;
        ddl_hook()?;
        if !is_v3(&transaction)? {
            return Err(migration_error(
                "migration produced invalid v3 catalog schema",
            ));
        }
    }
    if matches!(
        kind,
        SchemaKind::Fresh | SchemaKind::LegacyV1 | SchemaKind::CurrentV2 | SchemaKind::CurrentV3
    ) {
        transaction.execute_batch(TOOL_API_V4_SCHEMA)?;
        if !is_v4(&transaction)? {
            return Err(migration_error(
                "migration produced invalid v4 catalog schema",
            ));
        }
    }
    if matches!(
        kind,
        SchemaKind::Fresh
            | SchemaKind::LegacyV1
            | SchemaKind::CurrentV2
            | SchemaKind::CurrentV3
            | SchemaKind::CurrentV4
    ) {
        transaction.execute_batch(V5_SCHEMA)?;
        if !is_v5(&transaction)? {
            return Err(migration_error(
                "migration produced invalid v5 catalog schema",
            ));
        }
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
