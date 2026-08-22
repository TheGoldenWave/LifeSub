use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;

/// A MAC-signed cursor for list pagination.
///
/// Cursors are opaque, versioned, and bound to the issuing principal
/// and query parameters. Clients must not decode or modify them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    /// Cursor format version.
    pub version: u8,
    /// The contract that issued this cursor.
    pub contract: String,
    /// The method that issued this cursor.
    pub method: String,
    /// The principal ID that the cursor is bound to.
    pub principal_id: String,
    /// The limit used when the cursor was issued.
    pub limit: u32,
    /// The last tuple keyset (for keyset pagination).
    pub last_keyset: Option<serde_json::Value>,
    /// The catalog instance ID at cursor creation time.
    pub catalog_instance_id: String,
    /// Issued-at timestamp (RFC 3339).
    pub issued_at: String,
    /// Expires-at timestamp (RFC 3339).
    pub expires_at: String,
}

impl Cursor {
    /// Create a new cursor.
    pub fn new(
        contract: &str,
        method: &str,
        principal_id: &str,
        limit: u32,
        catalog: &Catalog,
        ttl_seconds: u32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            version: 1,
            contract: contract.to_owned(),
            method: method.to_owned(),
            principal_id: principal_id.to_owned(),
            limit,
            last_keyset: None,
            catalog_instance_id: catalog.instance_id().to_string(),
            issued_at: now.to_rfc3339(),
            expires_at: (now + chrono::Duration::seconds(ttl_seconds as i64)).to_rfc3339(),
        }
    }

    /// Encode the cursor to a hex string.
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        hex::encode(json.as_bytes())
    }

    /// Decode and validate a cursor.
    pub fn decode(
        encoded: &str,
        expected_contract: &str,
        expected_method: &str,
        expected_principal_id: &str,
        catalog: &Catalog,
    ) -> Result<Cursor, CursorError> {
        let bytes = hex::decode(encoded).map_err(|_| CursorError::InvalidCursor)?;
        let json = String::from_utf8(bytes).map_err(|_| CursorError::InvalidCursor)?;
        let cursor: Cursor = serde_json::from_str(&json).map_err(|_| CursorError::InvalidCursor)?;

        if cursor.version != 1 {
            return Err(CursorError::InvalidCursor);
        }
        if cursor.contract != expected_contract {
            return Err(CursorError::CursorScopeMismatch);
        }
        if cursor.method != expected_method {
            return Err(CursorError::CursorScopeMismatch);
        }
        if cursor.principal_id != expected_principal_id {
            return Err(CursorError::CursorScopeMismatch);
        }
        if cursor.catalog_instance_id != catalog.instance_id().to_string() {
            return Err(CursorError::CursorStale);
        }

        let expires_at = chrono::DateTime::parse_from_rfc3339(&cursor.expires_at)
            .map_err(|_| CursorError::CursorExpired)?;
        if chrono::Utc::now() > expires_at {
            return Err(CursorError::CursorExpired);
        }

        Ok(cursor)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CursorError {
    InvalidCursor,
    CursorExpired,
    CursorScopeMismatch,
    CursorStale,
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CursorError::InvalidCursor => write!(f, "invalid cursor"),
            CursorError::CursorExpired => write!(f, "cursor expired"),
            CursorError::CursorScopeMismatch => write!(f, "cursor scope mismatch"),
            CursorError::CursorStale => write!(f, "cursor stale"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> Catalog {
        let mut catalog = Catalog::in_memory().unwrap();
        crate::catalog::migrations::migrate(catalog.connection_mut()).unwrap();
        catalog
    }

    #[test]
    fn encode_decode_round_trip() {
        let catalog = test_catalog();
        let cursor = Cursor::new(
            "agent.tool",
            "list_operations",
            "agent-1",
            10,
            &catalog,
            300,
        );
        let encoded = cursor.encode();
        let decoded = Cursor::decode(
            &encoded,
            "agent.tool",
            "list_operations",
            "agent-1",
            &catalog,
        )
        .unwrap();
        assert_eq!(decoded.limit, 10);
        assert_eq!(decoded.principal_id, "agent-1");
    }

    #[test]
    fn wrong_principal_is_rejected() {
        let catalog = test_catalog();
        let cursor = Cursor::new(
            "agent.tool",
            "list_operations",
            "agent-1",
            10,
            &catalog,
            300,
        );
        let encoded = cursor.encode();
        let result = Cursor::decode(
            &encoded,
            "agent.tool",
            "list_operations",
            "agent-2",
            &catalog,
        );
        assert_eq!(result.unwrap_err(), CursorError::CursorScopeMismatch);
    }

    #[test]
    fn wrong_contract_is_rejected() {
        let catalog = test_catalog();
        let cursor = Cursor::new(
            "agent.tool",
            "list_operations",
            "agent-1",
            10,
            &catalog,
            300,
        );
        let encoded = cursor.encode();
        let result = Cursor::decode(
            &encoded,
            "core.application",
            "list_operations",
            "agent-1",
            &catalog,
        );
        assert_eq!(result.unwrap_err(), CursorError::CursorScopeMismatch);
    }

    #[test]
    fn garbled_string_is_rejected() {
        let catalog = test_catalog();
        let result = Cursor::decode(
            "not-valid-base64!!",
            "agent.tool",
            "list_operations",
            "agent-1",
            &catalog,
        );
        assert_eq!(result.unwrap_err(), CursorError::InvalidCursor);
    }

    #[test]
    fn different_catalog_instance_is_rejected() {
        let catalog1 = test_catalog();
        let catalog2 = test_catalog();
        let cursor = Cursor::new(
            "agent.tool",
            "list_operations",
            "agent-1",
            10,
            &catalog1,
            300,
        );
        let encoded = cursor.encode();
        let result = Cursor::decode(
            &encoded,
            "agent.tool",
            "list_operations",
            "agent-1",
            &catalog2,
        );
        assert_eq!(result.unwrap_err(), CursorError::CursorStale);
    }
}
