use std::collections::BTreeSet;

use rusqlite::Connection;

use super::ddl::{
    ASR_SCHEMA, CURRENT_VERSION, FRESH_BASE_SCHEMA, FTS_SHADOW_SCHEMA, FTS_SHADOWS, FTS_TABLE,
    LEGACY_SCHEMA, MODEL_MANAGER_V3_SCHEMA, TOOL_API_V4_SCHEMA, V1_TABLES, V2_TABLES, V3_TABLES,
    V4_TABLES, V5_SCHEMA, V5_TABLES,
};
use super::{SchemaKind, migration_error};

pub(super) fn classify_locked(connection: &Connection) -> rusqlite::Result<SchemaKind> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        0 if all_tables(connection)?.is_empty()
            && named_indexes(connection)?.is_empty()
            && auxiliary_objects(connection)?.is_empty() =>
        {
            Ok(SchemaKind::Fresh)
        }
        0 if is_v1(connection)? => Ok(SchemaKind::LegacyV1),
        0 => Ok(SchemaKind::Unknown),
        2 if is_v2(connection)? => Ok(SchemaKind::CurrentV2),
        2 => Err(migration_error("corrupt v2 catalog schema")),
        3 if is_v3(connection)? => Ok(SchemaKind::CurrentV3),
        3 => Err(migration_error("corrupt v3 catalog schema")),
        4 if is_v4(connection)? => Ok(SchemaKind::CurrentV4),
        4 => Err(migration_error("corrupt v4 catalog schema")),
        CURRENT_VERSION if is_v5(connection)? => Ok(SchemaKind::CurrentV5),
        CURRENT_VERSION => Err(migration_error("corrupt v5 catalog schema")),
        other => Err(migration_error(&format!(
            "incompatible catalog version {other}"
        ))),
    }
}

fn is_v1(connection: &Connection) -> rusqlite::Result<bool> {
    if all_tables(connection)? != table_names_with_fts_shadows(&V1_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_exact(connection)?
        || !named_indexes(connection)?.is_empty()
        || !auxiliary_objects(connection)?.is_empty()
    {
        return Ok(false);
    }
    for table in V1_TABLES {
        let expected = statement_named(LEGACY_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn is_v2(connection: &Connection) -> rusqlite::Result<bool> {
    let expected_indexes = names(&[
        "asr_jobs_claimable",
        "asr_jobs_one_active_fingerprint",
        "model_downloads_one_active_model",
    ]);
    if all_tables(connection)? != table_names_with_fts_shadows(&V2_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_exact(connection)?
        || named_indexes(connection)? != expected_indexes
        || !auxiliary_objects(connection)?.is_empty()
        || !base_tables_are_exact(connection)?
    {
        return Ok(false);
    }
    for table in [
        "asr_settings",
        "model_installations",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let expected = statement_named(ASR_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let expected = statement_named(ASR_SCHEMA, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn is_v3(connection: &Connection) -> rusqlite::Result<bool> {
    let expected_indexes = names(&[
        "asr_jobs_claimable",
        "asr_jobs_one_active_fingerprint",
        "model_download_artifacts_state",
        "model_downloads_one_active_model",
    ]);
    if all_tables(connection)? != table_names_with_fts_shadows(&V3_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_exact(connection)?
        || named_indexes(connection)? != expected_indexes
        || !auxiliary_objects(connection)?.is_empty()
        || !base_tables_are_exact(connection)?
    {
        return Ok(false);
    }
    for table in [
        "asr_settings",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let expected = statement_named(ASR_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in ["model_installations", "model_download_artifacts"] {
        let expected = statement_named(MODEL_MANAGER_V3_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let expected = statement_named(ASR_SCHEMA, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    let expected = statement_named(MODEL_MANAGER_V3_SCHEMA, "model_download_artifacts_state")?;
    let Some(actual) = schema_sql_optional(connection, "model_download_artifacts_state")? else {
        return Ok(false);
    };
    Ok(normalize_sql(&actual) == normalize_sql(&expected))
}

pub(super) fn is_v4(connection: &Connection) -> rusqlite::Result<bool> {
    let expected_indexes = names(&[
        "asr_jobs_claimable",
        "asr_jobs_one_active_fingerprint",
        "model_download_artifacts_state",
        "model_downloads_one_active_model",
        "operations_principal",
        "tool_requests_operation",
    ]);
    if all_tables(connection)? != table_names_with_fts_shadows(&V4_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_exact(connection)?
        || named_indexes(connection)? != expected_indexes
        || !auxiliary_objects(connection)?.is_empty()
        || !base_tables_are_exact(connection)?
    {
        return Ok(false);
    }
    for table in [
        "asr_settings",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let expected = statement_named(ASR_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in ["model_installations", "model_download_artifacts"] {
        let expected = statement_named(MODEL_MANAGER_V3_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in ["tool_requests", "operations", "open_intent_ledger"] {
        let expected = statement_named(TOOL_API_V4_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let expected = statement_named(ASR_SCHEMA, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for (schema, index) in [
        (MODEL_MANAGER_V3_SCHEMA, "model_download_artifacts_state"),
        (TOOL_API_V4_SCHEMA, "operations_principal"),
        (TOOL_API_V4_SCHEMA, "tool_requests_operation"),
    ] {
        let expected = statement_named(schema, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn is_v5(connection: &Connection) -> rusqlite::Result<bool> {
    let expected_indexes = names(&[
        "asr_jobs_claimable",
        "asr_jobs_one_active_fingerprint",
        "dictionary_entries_category",
        "model_download_artifacts_state",
        "model_downloads_one_active_model",
        "notes_session",
        "operations_principal",
        "tool_requests_operation",
    ]);
    if all_tables(connection)? != table_names_with_fts_shadows(&V5_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_exact(connection)?
        || named_indexes(connection)? != expected_indexes
        || !auxiliary_objects(connection)?.is_empty()
        || !base_tables_are_exact(connection)?
    {
        return Ok(false);
    }
    for table in [
        "asr_settings",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let expected = statement_named(ASR_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in ["model_installations", "model_download_artifacts"] {
        let expected = statement_named(MODEL_MANAGER_V3_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in ["tool_requests", "operations", "open_intent_ledger"] {
        let expected = statement_named(TOOL_API_V4_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for table in [
        "notes",
        "dictionary_categories",
        "dictionary_entries",
        "voiceprints",
        "settings",
    ] {
        let expected = statement_named(V5_SCHEMA, table)?;
        if normalize_sql(&schema_sql(connection, table)?) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let expected = statement_named(ASR_SCHEMA, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    for (schema, index) in [
        (MODEL_MANAGER_V3_SCHEMA, "model_download_artifacts_state"),
        (TOOL_API_V4_SCHEMA, "operations_principal"),
        (TOOL_API_V4_SCHEMA, "tool_requests_operation"),
        (V5_SCHEMA, "notes_session"),
        (V5_SCHEMA, "dictionary_entries_category"),
    ] {
        let expected = statement_named(schema, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn base_tables_are_exact(connection: &Connection) -> rusqlite::Result<bool> {
    for table in ["sessions", "revisions", "segments", "chunks"] {
        if normalize_sql(&schema_sql(connection, table)?)
            != normalize_sql(&statement_named(FRESH_BASE_SCHEMA, table)?)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fts_shadows_are_exact(connection: &Connection) -> rusqlite::Result<bool> {
    for (name, expected) in FTS_SHADOW_SCHEMA {
        let Some(actual) = schema_sql_optional(connection, name)? else {
            return Ok(false);
        };
        if normalize_sql(&actual) != normalize_sql(expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fts_is_trigram(connection: &Connection) -> rusqlite::Result<bool> {
    let expected = "CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram')";
    Ok(normalize_sql(&schema_sql(connection, FTS_TABLE)?) == normalize_sql(expected))
}

fn all_tables(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    schema_names(connection, "table")
}
fn named_indexes(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    schema_names(connection, "index")
}

fn auxiliary_objects(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    let mut statement = connection.prepare("SELECT name FROM sqlite_schema WHERE type IN ('view', 'trigger') AND name NOT LIKE 'sqlite_%'")?;
    statement.query_map([], |row| row.get(0))?.collect()
}

fn schema_names(connection: &Connection, object_type: &str) -> rusqlite::Result<BTreeSet<String>> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = ?1 AND name NOT LIKE 'sqlite_%'")?;
    statement
        .query_map([object_type], |row| row.get(0))?
        .collect()
}

fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn table_names_with_fts_shadows(values: &[&str]) -> BTreeSet<String> {
    values
        .iter()
        .chain(FTS_SHADOWS.iter())
        .map(|value| (*value).to_owned())
        .collect()
}

fn schema_sql(connection: &Connection, name: &str) -> rusqlite::Result<String> {
    connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE name = ?1",
        [name],
        |row| row.get(0),
    )
}

fn schema_sql_optional(connection: &Connection, name: &str) -> rusqlite::Result<Option<String>> {
    let mut statement = connection.prepare("SELECT sql FROM sqlite_schema WHERE name = ?1")?;
    let mut rows = statement.query([name])?;
    rows.next()?.map(|row| row.get(0)).transpose()
}

fn statement_named(sql: &str, name: &str) -> rusqlite::Result<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .find(|statement| {
            let normalized = normalize_sql(statement);
            normalized.starts_with(&format!("create table {name}("))
                || normalized.starts_with(&format!("create virtual table {name}"))
                || normalized.starts_with(&format!("create unique index {name}"))
                || normalized.starts_with(&format!("create index {name}"))
        })
        .map(str::to_owned)
        .ok_or_else(|| migration_error(&format!("missing schema contract for {name}")))
}

pub(super) fn normalize_sql(sql: &str) -> String {
    let mut normalized = String::with_capacity(sql.len());
    let mut characters = sql.chars().peekable();
    let mut in_literal = false;
    let mut pending_whitespace = false;
    while let Some(character) = characters.next() {
        if in_literal {
            normalized.push(character);
            if character == '\'' {
                if characters.peek() == Some(&'\'') {
                    normalized.push(characters.next().unwrap());
                } else {
                    in_literal = false;
                }
            }
        } else if character == '\'' {
            in_literal = true;
            normalized.push(character);
            pending_whitespace = false;
        } else if character.is_ascii_whitespace() {
            pending_whitespace = true;
        } else {
            if pending_whitespace
                && normalized.chars().last().is_some_and(is_token_character)
                && is_token_character(character)
            {
                normalized.push(' ');
            }
            normalized.extend(character.to_lowercase());
            pending_whitespace = false;
        }
    }
    normalized
}

fn is_token_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
