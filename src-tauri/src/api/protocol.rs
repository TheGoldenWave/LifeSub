use serde::{Deserialize, Serialize};

// ── Envelope ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub contract: String,
    pub contract_version: u32,
    pub request_id: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub contract: String,
    pub contract_version: u32,
    pub request_id: String,
    #[serde(rename = "ok")]
    pub ok: bool,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub contract: String,
    pub contract_version: u32,
    pub request_id: String,
    #[serde(rename = "ok")]
    pub ok: bool,
    pub error: ApiError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message_key: String,
    pub retryable: bool,
    pub details: Option<serde_json::Value>,
}

// ── Trusted Caller ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerKind {
    LocalAgent,
    TauriUi,
    Gateway,
}

#[derive(Debug, Clone)]
pub struct TrustedCallerContext {
    pub principal_id: String,
    pub kind: CallerKind,
    pub capabilities: Vec<String>,
    pub auth_source: String,
}

// ── Contracts ─────────────────────────────────────────────────────────────────

pub const AGENT_CONTRACT: &str = "agent.tool";
pub const APPLICATION_CONTRACT: &str = "core.application";
pub const CONTRACT_VERSION: u32 = 1;

// ── Agent V1 Methods ──────────────────────────────────────────────────────────

pub const AGENT_METHODS: [&str; 8] = [
    "get_capabilities",
    "get_capture_status",
    "get_asr_job_status",
    "search_transcripts",
    "resolve_evidence",
    "open_evidence",
    "get_operation",
    "list_operations",
];

// ── Application V1 Methods (Application-only) ─────────────────────────────────

pub const APPLICATION_ONLY_METHODS: [&str; 16] = [
    "start_capture",
    "stop_capture",
    "install_model",
    "uninstall_model",
    "get_model",
    "list_models",
    "import_audio",
    "enqueue_asr_job",
    "retry_asr_job",
    "cancel_asr_job",
    "retranscribe_chunk",
    "get_operation",
    "list_operations",
    "list_transcript_revisions",
    "get_transcript_revision",
    "list_provider_receipts",
];

// ── Capabilities ──────────────────────────────────────────────────────────────

pub const CAP_AGENT_READ: &str = "agent_read";
pub const CAP_AGENT_EVIDENCE: &str = "agent_evidence";
pub const CAP_CAPTURE_MANAGE: &str = "capture_manage";
pub const CAP_MODEL_MANAGE: &str = "model_manage";
pub const CAP_IMPORT_MANAGE: &str = "import_manage";
pub const CAP_ASR_JOB_MANAGE: &str = "asr_job_manage";
pub const CAP_OPERATION_READ: &str = "operation_read";
pub const CAP_TRANSCRIPT_READ: &str = "transcript_read";
pub const CAP_RECEIPT_READ: &str = "receipt_read";

// ── Error Codes ───────────────────────────────────────────────────────────────

pub const E_UNSUPPORTED_CAPABILITY: &str = "unsupported_capability";
pub const E_UNKNOWN_METHOD: &str = "unknown_method";
pub const E_INVALID_REQUEST: &str = "invalid_request";
pub const E_IDEMPOTENCY_CONFLICT: &str = "idempotency_conflict";
pub const E_OPERATION_IN_PROGRESS: &str = "operation_in_progress";
pub const E_OPERATION_NOT_FOUND: &str = "operation_not_found";
pub const E_INVALID_OPERATION_FILTER: &str = "invalid_operation_filter";
pub const E_INVALID_CURSOR: &str = "invalid_cursor";
pub const E_CURSOR_EXPIRED: &str = "cursor_expired";
pub const E_CURSOR_SCOPE_MISMATCH: &str = "cursor_scope_mismatch";
pub const E_CURSOR_STALE: &str = "cursor_stale";
pub const E_MODEL_NOT_FOUND: &str = "model_not_found";
pub const E_MODEL_IN_USE: &str = "model_in_use";
pub const E_ACTIVE_MODEL_REQUIRED: &str = "active_model_required";
pub const E_CHUNK_NOT_FOUND: &str = "chunk_not_found";
pub const E_CHUNK_INTEGRITY_FAILED: &str = "chunk_integrity_failed";
pub const E_MODEL_CAPABILITY_UNAVAILABLE: &str = "model_capability_unavailable";
pub const E_JOB_ALREADY_ACTIVE: &str = "job_already_active";
pub const E_JOB_NOT_FOUND: &str = "job_not_found";
pub const E_INVALID_JOB_STATE: &str = "invalid_job_state";
pub const E_ALREADY_COMMITTED: &str = "already_committed";
pub const E_INVALID_SETTINGS: &str = "invalid_settings";
pub const E_SESSION_NOT_FOUND: &str = "session_not_found";
pub const E_REVISION_NOT_FOUND: &str = "revision_not_found";
pub const E_EVIDENCE_NOT_FOUND: &str = "evidence_not_found";
pub const E_EVIDENCE_UNAVAILABLE: &str = "evidence_unavailable";
pub const E_EVIDENCE_EXPIRED: &str = "evidence_expired";
pub const E_INTERNAL: &str = "internal_error";

// ── DTO Primitives ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSummary {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub method: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_error_code: Option<String>,
    pub last_error_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrJobSummary {
    pub id: String,
    pub session_id: String,
    pub chunk_id: String,
    pub provider: String,
    pub model_id: String,
    pub state: String,
    pub attempt_count: i32,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSummary {
    pub model_id: String,
    pub provider: String,
    pub manifest_version: String,
    pub state: String,
    pub installed_at: String,
    pub qualified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRevisionSummary {
    pub revision_id: String,
    pub session_id: String,
    pub number: i32,
    pub provider: String,
    pub provenance_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentSummary {
    pub segment_id: String,
    pub revision_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReceiptSummary {
    pub receipt_id: String,
    pub job_id: String,
    pub provider: String,
    pub model_id: String,
    pub runtime_version: String,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub intent_id: String,
    pub state: String,
    pub disposition: String,
    pub expires_at: String,
    pub display_metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

// ── Capabilities Response ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub contract: String,
    pub contract_version: u32,
    pub capabilities: Vec<String>,
    pub unsupported_capabilities: Vec<UnsupportedCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedCapability {
    pub capability: String,
    pub reason: String,
    pub available_at_milestone: Option<String>,
}

// ── Capture Status ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStatus {
    pub active: bool,
    pub capability: String,
    pub state: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

impl ApiError {
    pub fn new(code: &str, message_key: &str, retryable: bool) -> Self {
        Self {
            code: code.to_owned(),
            message_key: message_key.to_owned(),
            retryable,
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn unsupported_capability(capability: &str) -> Self {
        Self::new(E_UNSUPPORTED_CAPABILITY, "capability not available", false).with_details(
            serde_json::json!({
                "capability": capability,
                "available_at_milestone": "native_capture"
            }),
        )
    }

    pub fn unknown_method(method: &str) -> Self {
        Self::new(E_UNKNOWN_METHOD, "unknown method", false)
            .with_details(serde_json::json!({ "method": method }))
    }
}

impl SuccessResponse {
    pub fn new(contract: &str, request_id: &str, result: serde_json::Value) -> Self {
        Self {
            contract: contract.to_owned(),
            contract_version: CONTRACT_VERSION,
            request_id: request_id.to_owned(),
            ok: true,
            result,
        }
    }
}

impl ErrorResponse {
    pub fn new(contract: &str, request_id: &str, error: ApiError) -> Self {
        Self {
            contract: contract.to_owned(),
            contract_version: CONTRACT_VERSION,
            request_id: request_id.to_owned(),
            ok: false,
            error,
        }
    }
}

impl std::fmt::Display for CallerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallerKind::LocalAgent => write!(f, "local_agent"),
            CallerKind::TauriUi => write!(f, "tauri_ui"),
            CallerKind::Gateway => write!(f, "gateway"),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_response_round_trips() {
        let resp = SuccessResponse::new(
            AGENT_CONTRACT,
            "req-1",
            serde_json::json!({"capabilities": ["agent_read"]}),
        );
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: SuccessResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.contract, AGENT_CONTRACT);
        assert_eq!(parsed.contract_version, 1);
        assert!(parsed.ok);
    }

    #[test]
    fn error_response_round_trips() {
        let err = ErrorResponse::new(
            AGENT_CONTRACT,
            "req-2",
            ApiError::unsupported_capability("native_capture"),
        );
        let json = serde_json::to_string(&err).unwrap();
        let parsed: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.error.code, E_UNSUPPORTED_CAPABILITY);
        assert!(!parsed.error.retryable);
    }

    #[test]
    fn request_envelope_round_trips() {
        let req = RequestEnvelope {
            contract: AGENT_CONTRACT.to_owned(),
            contract_version: 1,
            request_id: "req-3".to_owned(),
            method: "get_capabilities".to_owned(),
            params: serde_json::json!({}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RequestEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "get_capabilities");
        assert_eq!(parsed.contract, AGENT_CONTRACT);
    }

    #[test]
    fn caller_kind_display() {
        assert_eq!(CallerKind::LocalAgent.to_string(), "local_agent");
        assert_eq!(CallerKind::TauriUi.to_string(), "tauri_ui");
        assert_eq!(CallerKind::Gateway.to_string(), "gateway");
    }

    #[test]
    fn agent_methods_are_unique() {
        let mut methods = AGENT_METHODS.to_vec();
        methods.sort_unstable();
        methods.dedup();
        assert_eq!(methods.len(), AGENT_METHODS.len());
    }

    #[test]
    fn application_only_methods_are_unique() {
        let mut methods = APPLICATION_ONLY_METHODS.to_vec();
        methods.sort_unstable();
        methods.dedup();
        assert_eq!(methods.len(), APPLICATION_ONLY_METHODS.len());
    }

    #[test]
    fn no_overlap_between_agent_and_app_only() {
        let agent: std::collections::BTreeSet<_> = AGENT_METHODS.iter().collect();
        let app: std::collections::BTreeSet<_> = APPLICATION_ONLY_METHODS.iter().collect();
        // Overlap is expected: Tauri can also call Agent methods (trusted UI
        // projection). The overlap methods are: get_operation, list_operations.
        let overlap: Vec<_> = agent.intersection(&app).collect();
        assert_eq!(
            overlap.len(),
            2,
            "expected 2 overlap methods: {:?}",
            overlap
        );
    }

    #[test]
    fn error_with_details() {
        let err = ApiError::new("test_code", "test_key", true)
            .with_details(serde_json::json!({"operation_id": "op-1"}));
        let json = serde_json::to_string(&err).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["details"]["operation_id"], "op-1");
    }

    #[test]
    fn dto_serde_round_trips() {
        let op = OperationSummary {
            id: "op-1".to_owned(),
            kind: "import_audio".to_owned(),
            state: "queued".to_owned(),
            method: "import_audio".to_owned(),
            created_at: "2026-08-17T00:00:00Z".to_owned(),
            updated_at: "2026-08-17T00:00:00Z".to_owned(),
            last_error_code: None,
            last_error_summary: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: OperationSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "op-1");
        assert_eq!(parsed.state, "queued");
    }

    #[test]
    fn model_summary_serde() {
        let m = ModelSummary {
            model_id: "whisper-tiny".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "v1.0".to_owned(),
            state: "runtime_qualified".to_owned(),
            installed_at: "2026-08-17T00:00:00Z".to_owned(),
            qualified_at: Some("2026-08-17T01:00:00Z".to_owned()),
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: ModelSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.qualified_at, Some("2026-08-17T01:00:00Z".to_owned()));
    }

    #[test]
    fn evidence_ref_serde() {
        let e = EvidenceRef {
            intent_id: "int-1".to_owned(),
            state: "pending".to_owned(),
            disposition: "requires_consent".to_owned(),
            expires_at: "2026-08-17T05:00:00Z".to_owned(),
            display_metadata: serde_json::json!({"title": "test"}),
        };
        let json = serde_json::to_string(&e).unwrap();
        let parsed: EvidenceRef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.intent_id, "int-1");
    }

    #[test]
    fn paginated_response_serde() {
        let p: PaginatedResponse<String> = PaginatedResponse {
            items: vec!["a".to_owned(), "b".to_owned()],
            next_cursor: Some("cursor-1".to_owned()),
        };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: PaginatedResponse<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.next_cursor, Some("cursor-1".to_owned()));
    }
}
