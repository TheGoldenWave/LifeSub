use crate::api::protocol::TrustedCallerContext;
use crate::catalog::Catalog;

/// Result of an idempotency check.
pub enum IdempotencyResult {
    /// No existing request; proceed with the mutation.
    Proceed,
    /// An existing request is in progress; return this error.
    InProgress { operation_id: String },
    /// A previous request succeeded; return the cached response.
    Succeeded { response_json: String },
    /// A previous request failed; return the cached error.
    Failed {
        error_code: String,
        error_message_key: String,
    },
}

/// Errors from the idempotency layer.
#[derive(Debug)]
pub enum IdempotencyError {
    Catalog(rusqlite::Error),
}

impl From<rusqlite::Error> for IdempotencyError {
    fn from(e: rusqlite::Error) -> Self {
        IdempotencyError::Catalog(e)
    }
}

/// Checks or claims an idempotency key for a mutation request.
///
/// If the key has been seen before, returns the cached result.
/// Otherwise, inserts a new `in_progress` row and returns `Proceed`.
///
/// The caller must call `commit()` or `fail()` after the mutation completes.
#[allow(clippy::type_complexity)]
pub fn check_or_claim(
    catalog: &Catalog,
    contract: &str,
    contract_version: u32,
    caller: &TrustedCallerContext,
    method: &str,
    idempotency_key: &str,
    request_fingerprint: &str,
) -> Result<IdempotencyResult, IdempotencyError> {
    let connection = catalog.connection();

    // Check if the key already exists.
    let existing: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = connection
        .query_row(
            "SELECT state, operation_id, response_json, error_code, error_message_key
                 FROM tool_requests
                 WHERE contract = ?1 AND contract_version = ?2
                   AND principal_id = ?3 AND idempotency_key = ?4",
            rusqlite::params![
                contract,
                contract_version,
                caller.principal_id,
                idempotency_key
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .ok();

    match existing {
        Some((
            ref state,
            ref operation_id,
            ref response_json,
            ref error_code,
            ref error_message_key,
        )) => {
            match state.as_str() {
                "in_progress" => Ok(IdempotencyResult::InProgress {
                    operation_id: operation_id.clone(),
                }),
                "succeeded" => Ok(IdempotencyResult::Succeeded {
                    response_json: response_json.clone().unwrap_or_default(),
                }),
                "failed" => Ok(IdempotencyResult::Failed {
                    error_code: error_code.clone().unwrap_or_default(),
                    error_message_key: error_message_key.clone().unwrap_or_default(),
                }),
                _ => {
                    // Unknown state — treat as conflict.
                    Ok(IdempotencyResult::InProgress {
                        operation_id: operation_id.clone(),
                    })
                }
            }
        }
        None => {
            // Insert a new in_progress row.
            let now = chrono::Utc::now().to_rfc3339();
            connection.execute(
                "INSERT INTO tool_requests
                    (idempotency_key, contract, contract_version, principal_id, principal_kind,
                     method, request_fingerprint, state, created_at, committed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'in_progress', ?8, ?8)",
                rusqlite::params![
                    idempotency_key,
                    contract,
                    contract_version,
                    caller.principal_id,
                    caller.kind.to_string(),
                    method,
                    request_fingerprint,
                    now,
                ],
            )?;
            Ok(IdempotencyResult::Proceed)
        }
    }
}

/// Commit a successful idempotency key with the response.
pub fn commit_success(
    catalog: &Catalog,
    contract: &str,
    contract_version: u32,
    principal_id: &str,
    idempotency_key: &str,
    response_json: &str,
    operation_id: Option<&str>,
) -> Result<(), IdempotencyError> {
    let now = chrono::Utc::now().to_rfc3339();
    catalog.connection().execute(
        "UPDATE tool_requests
         SET state = 'succeeded', response_json = ?1, operation_id = ?2, committed_at = ?3
         WHERE contract = ?4 AND contract_version = ?5
           AND principal_id = ?6 AND idempotency_key = ?7",
        rusqlite::params![
            response_json,
            operation_id,
            now,
            contract,
            contract_version,
            principal_id,
            idempotency_key,
        ],
    )?;
    Ok(())
}

/// Mark an idempotency key as failed.
pub fn commit_failure(
    catalog: &Catalog,
    contract: &str,
    contract_version: u32,
    principal_id: &str,
    idempotency_key: &str,
    error_code: &str,
    error_message_key: &str,
) -> Result<(), IdempotencyError> {
    let now = chrono::Utc::now().to_rfc3339();
    catalog.connection().execute(
        "UPDATE tool_requests
         SET state = 'failed', error_code = ?1, error_message_key = ?2, committed_at = ?3
         WHERE contract = ?4 AND contract_version = ?5
           AND principal_id = ?6 AND idempotency_key = ?7",
        rusqlite::params![
            error_code,
            error_message_key,
            now,
            contract,
            contract_version,
            principal_id,
            idempotency_key,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::protocol::CallerKind;

    fn test_catalog() -> Catalog {
        let mut catalog = Catalog::in_memory().unwrap();
        crate::catalog::migrations::migrate(catalog.connection_mut()).unwrap();
        catalog
    }

    fn agent_caller() -> TrustedCallerContext {
        TrustedCallerContext {
            principal_id: "agent-1".to_owned(),
            kind: CallerKind::LocalAgent,
            capabilities: vec!["agent_read".to_owned()],
            auth_source: "test".to_owned(),
        }
    }

    #[test]
    fn first_request_proceeds() {
        let catalog = test_catalog();
        let result = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &agent_caller(),
            "import_audio",
            "key-1",
            "fp-1",
        )
        .unwrap();
        assert!(matches!(result, IdempotencyResult::Proceed));
    }

    #[test]
    fn duplicate_request_returns_in_progress() {
        let catalog = test_catalog();
        let caller = agent_caller();

        let _ = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-2",
            "fp-2",
        )
        .unwrap();

        let result = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-2",
            "fp-2",
        )
        .unwrap();
        assert!(matches!(result, IdempotencyResult::InProgress { .. }));
    }

    #[test]
    fn commit_success_caches_response() {
        let catalog = test_catalog();
        let caller = agent_caller();

        let _ = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-3",
            "fp-3",
        )
        .unwrap();

        commit_success(
            &catalog,
            "agent.tool",
            1,
            &caller.principal_id,
            "key-3",
            "{\"ok\":true}",
            None,
        )
        .unwrap();

        let result = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-3",
            "fp-3",
        )
        .unwrap();
        assert!(matches!(result, IdempotencyResult::Succeeded { .. }));
    }

    #[test]
    fn commit_failure_caches_error() {
        let catalog = test_catalog();
        let caller = agent_caller();

        let _ = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-4",
            "fp-4",
        )
        .unwrap();

        commit_failure(
            &catalog,
            "agent.tool",
            1,
            &caller.principal_id,
            "key-4",
            "model_not_found",
            "model not found",
        )
        .unwrap();

        let result = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-4",
            "fp-4",
        )
        .unwrap();
        assert!(matches!(result, IdempotencyResult::Failed { .. }));
    }

    #[test]
    fn different_principals_are_independent() {
        let catalog = test_catalog();
        let caller1 = agent_caller();
        let caller2 = TrustedCallerContext {
            principal_id: "agent-2".to_owned(),
            kind: CallerKind::LocalAgent,
            capabilities: vec!["agent_read".to_owned()],
            auth_source: "test".to_owned(),
        };

        let r1 = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller1,
            "import_audio",
            "key-shared",
            "fp",
        )
        .unwrap();
        assert!(matches!(r1, IdempotencyResult::Proceed));

        let r2 = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller2,
            "import_audio",
            "key-shared",
            "fp",
        )
        .unwrap();
        assert!(matches!(r2, IdempotencyResult::Proceed));
    }

    #[test]
    fn different_contracts_are_independent() {
        let catalog = test_catalog();
        let caller = agent_caller();

        let _ = check_or_claim(
            &catalog,
            "agent.tool",
            1,
            &caller,
            "import_audio",
            "key-5",
            "fp",
        )
        .unwrap();

        let result = check_or_claim(
            &catalog,
            "core.application",
            1,
            &caller,
            "import_audio",
            "key-5",
            "fp",
        )
        .unwrap();
        assert!(matches!(result, IdempotencyResult::Proceed));
    }
}
