use crate::domain::AsrErrorCode;

#[derive(Debug)]
pub enum ServiceError {
    Io(std::io::Error),
    Catalog(rusqlite::Error),
    InvalidEvidenceUri,
    InputIntegrityFailed,
    InputUnavailable,
    #[cfg(test)]
    InjectedCrash(ImportFault),
}

impl PartialEq for ServiceError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::InvalidEvidenceUri, Self::InvalidEvidenceUri)
                | (Self::InputIntegrityFailed, Self::InputIntegrityFailed)
                | (Self::InputUnavailable, Self::InputUnavailable)
        ) || {
            #[cfg(test)]
            {
                matches!((self, other), (Self::InjectedCrash(left), Self::InjectedCrash(right)) if left == right)
            }
            #[cfg(not(test))]
            {
                false
            }
        }
    }
}

impl ServiceError {
    pub const fn code(&self) -> AsrErrorCode {
        match self {
            Self::InputIntegrityFailed => AsrErrorCode::InputIntegrityFailed,
            Self::InputUnavailable => AsrErrorCode::InputUnavailable,
            Self::Io(_) | Self::Catalog(_) | Self::InvalidEvidenceUri => {
                AsrErrorCode::RecoveryRequired
            }
            #[cfg(test)]
            Self::InjectedCrash(_) => AsrErrorCode::RecoveryRequired,
        }
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for ServiceError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Catalog(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportFault {
    AfterTempSync,
    AfterFinalRename,
    RenameIo,
    ParentSyncIo,
}
