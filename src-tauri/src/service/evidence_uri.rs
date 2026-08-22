use super::ServiceError;

#[derive(Debug, Eq, PartialEq)]
pub enum EvidenceTarget {
    Record {
        id: String,
    },
    Segment {
        id: String,
        revision: Option<i64>,
    },
    Audio {
        id: String,
        start_seconds: Option<i64>,
        end_seconds: Option<i64>,
    },
}

pub fn parse_evidence_uri(uri: &str) -> Result<EvidenceTarget, ServiceError> {
    let body = uri
        .strip_prefix("lifesub://")
        .ok_or(ServiceError::InvalidEvidenceUri)?;
    if let Some(id) = body.strip_prefix("record/") {
        return non_empty(id).map(|id| EvidenceTarget::Record { id });
    }
    if let Some(body) = body.strip_prefix("segment/") {
        let (id, query) = body.split_once('?').unwrap_or((body, ""));
        let revision = query
            .strip_prefix("revision=")
            .and_then(|value| value.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Segment { id, revision });
    }
    if let Some(body) = body.strip_prefix("audio/") {
        let (id, fragment) = body.split_once('#').unwrap_or((body, ""));
        let range = fragment
            .strip_prefix("t=")
            .and_then(|value| value.split_once(','));
        let start_seconds = range.and_then(|(start, _)| start.parse().ok());
        let end_seconds = range.and_then(|(_, end)| end.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Audio {
            id,
            start_seconds,
            end_seconds,
        });
    }
    Err(ServiceError::InvalidEvidenceUri)
}

fn non_empty(value: &str) -> Result<String, ServiceError> {
    if value.is_empty() {
        Err(ServiceError::InvalidEvidenceUri)
    } else {
        Ok(value.to_owned())
    }
}
