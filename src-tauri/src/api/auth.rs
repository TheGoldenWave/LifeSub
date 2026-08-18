use crate::api::protocol::{
    AGENT_CONTRACT, AGENT_METHODS, APPLICATION_CONTRACT, APPLICATION_ONLY_METHODS,
    CAP_AGENT_EVIDENCE, CAP_AGENT_READ, CAP_ASR_JOB_MANAGE, CAP_CAPTURE_MANAGE, CAP_IMPORT_MANAGE,
    CAP_MODEL_MANAGE, CAP_OPERATION_READ, CAP_RECEIPT_READ, CAP_TRANSCRIPT_READ, CallerKind,
    E_UNKNOWN_METHOD, E_UNSUPPORTED_CAPABILITY, TrustedCallerContext,
};

/// Result of authorizing a method call.
#[derive(Debug)]
pub enum AuthResult {
    Allowed,
    Denied {
        error_code: &'static str,
        message: &'static str,
    },
}

/// Authorize a method call for a given caller context.
///
/// Returns `AuthResult::Allowed` if the caller has the required capability
/// for the method. Returns `AuthResult::Denied` with a stable error code
/// otherwise. The caller must not be trusted for any self-reported fields.
pub fn authorize_method(contract: &str, method: &str, caller: &TrustedCallerContext) -> AuthResult {
    // Validate contract.
    match contract {
        AGENT_CONTRACT => {
            if !AGENT_METHODS.contains(&method) {
                return AuthResult::Denied {
                    error_code: E_UNKNOWN_METHOD,
                    message: "method not in agent contract",
                };
            }
        }
        APPLICATION_CONTRACT => {
            let all_app: std::collections::BTreeSet<_> = AGENT_METHODS
                .iter()
                .chain(APPLICATION_ONLY_METHODS.iter())
                .collect();
            if !all_app.contains(&method) {
                return AuthResult::Denied {
                    error_code: E_UNKNOWN_METHOD,
                    message: "method not in application contract",
                };
            }
        }
        _ => {
            return AuthResult::Denied {
                error_code: "unknown_contract",
                message: "unknown contract",
            };
        }
    }

    // Agent can only call Agent V1 methods; Application-only methods (those
    // NOT also in the Agent contract) are blocked.
    if caller.kind == CallerKind::LocalAgent
        && APPLICATION_ONLY_METHODS.contains(&method)
        && !AGENT_METHODS.contains(&method)
    {
        return AuthResult::Denied {
            error_code: E_UNSUPPORTED_CAPABILITY,
            message: "agent cannot call application-only methods",
        };
    }

    // Check capability.
    let required = required_capability(method);
    if !caller.capabilities.iter().any(|c| c == required) {
        return AuthResult::Denied {
            error_code: E_UNSUPPORTED_CAPABILITY,
            message: "missing required capability",
        };
    }

    AuthResult::Allowed
}

fn required_capability(method: &str) -> &str {
    match method {
        "get_capabilities" | "get_capture_status" | "get_asr_job_status" => CAP_AGENT_READ,
        "search_transcripts" => CAP_AGENT_READ,
        "resolve_evidence" | "open_evidence" => CAP_AGENT_EVIDENCE,
        "start_capture" | "stop_capture" => CAP_CAPTURE_MANAGE,
        "install_model" | "uninstall_model" | "get_model" | "list_models" => CAP_MODEL_MANAGE,
        "import_audio" => CAP_IMPORT_MANAGE,
        "enqueue_asr_job" | "retry_asr_job" | "cancel_asr_job" | "retranscribe_chunk" => {
            CAP_ASR_JOB_MANAGE
        }
        "get_operation" | "list_operations" => CAP_OPERATION_READ,
        "list_transcript_revisions" | "get_transcript_revision" => CAP_TRANSCRIPT_READ,
        "list_provider_receipts" => CAP_RECEIPT_READ,
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::protocol::CallerKind;

    fn agent_context() -> TrustedCallerContext {
        TrustedCallerContext {
            principal_id: "agent-1".to_owned(),
            kind: CallerKind::LocalAgent,
            capabilities: vec![
                CAP_AGENT_READ.to_owned(),
                CAP_AGENT_EVIDENCE.to_owned(),
                CAP_OPERATION_READ.to_owned(),
            ],
            auth_source: "test".to_owned(),
        }
    }

    fn tauri_context() -> TrustedCallerContext {
        TrustedCallerContext {
            principal_id: "tauri-1".to_owned(),
            kind: CallerKind::TauriUi,
            capabilities: vec![
                CAP_AGENT_READ.to_owned(),
                CAP_AGENT_EVIDENCE.to_owned(),
                CAP_OPERATION_READ.to_owned(),
                CAP_TRANSCRIPT_READ.to_owned(),
                CAP_RECEIPT_READ.to_owned(),
                CAP_CAPTURE_MANAGE.to_owned(),
                CAP_MODEL_MANAGE.to_owned(),
                CAP_IMPORT_MANAGE.to_owned(),
                CAP_ASR_JOB_MANAGE.to_owned(),
            ],
            auth_source: "test".to_owned(),
        }
    }

    #[test]
    fn agent_can_call_agent_methods() {
        let ctx = agent_context();
        for method in AGENT_METHODS {
            let result = authorize_method(AGENT_CONTRACT, method, &ctx);
            assert!(
                matches!(result, AuthResult::Allowed),
                "agent should be allowed {method}"
            );
        }
    }

    #[test]
    fn agent_cannot_call_application_only_methods() {
        let ctx = agent_context();
        let agent_set: std::collections::BTreeSet<_> = AGENT_METHODS.iter().collect();
        for method in APPLICATION_ONLY_METHODS {
            // Skip methods that overlap with agent contract (get_operation, list_operations).
            if agent_set.contains(&method) {
                continue;
            }
            let result = authorize_method(APPLICATION_CONTRACT, method, &ctx);
            assert!(
                matches!(result, AuthResult::Denied { .. }),
                "agent should be denied {method}"
            );
        }
    }

    #[test]
    fn tauri_can_call_all_methods() {
        let ctx = tauri_context();
        for method in AGENT_METHODS {
            let result = authorize_method(APPLICATION_CONTRACT, method, &ctx);
            assert!(
                matches!(result, AuthResult::Allowed),
                "tauri should be allowed {method}"
            );
        }
        for method in APPLICATION_ONLY_METHODS {
            let result = authorize_method(APPLICATION_CONTRACT, method, &ctx);
            assert!(
                matches!(result, AuthResult::Allowed),
                "tauri should be allowed {method}"
            );
        }
    }

    #[test]
    fn unknown_method_is_denied() {
        let ctx = tauri_context();
        let result = authorize_method(APPLICATION_CONTRACT, "delete_everything", &ctx);
        assert!(matches!(result, AuthResult::Denied { .. }));
    }

    #[test]
    fn unknown_contract_is_denied() {
        let ctx = tauri_context();
        let result = authorize_method("evil.contract", "get_capabilities", &ctx);
        assert!(matches!(result, AuthResult::Denied { .. }));
    }

    #[test]
    fn agent_without_capability_is_denied() {
        let ctx = TrustedCallerContext {
            principal_id: "minimal".to_owned(),
            kind: CallerKind::LocalAgent,
            capabilities: vec![], // no capabilities
            auth_source: "test".to_owned(),
        };
        let result = authorize_method(AGENT_CONTRACT, "search_transcripts", &ctx);
        assert!(matches!(result, AuthResult::Denied { .. }));
    }
}
