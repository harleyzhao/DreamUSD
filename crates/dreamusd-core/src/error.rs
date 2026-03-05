use dreamusd_sys::DuStatus;

#[derive(Debug, thiserror::Error)]
pub enum DuError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid operation: {0}")]
    Invalid(String),
    #[error("Null pointer")]
    Null,
    #[error("USD error: {0}")]
    Usd(String),
    #[error("Vulkan error: {0}")]
    Vulkan(String),
}

/// Check a DuStatus and convert non-Ok values to an error.
pub(crate) fn check(status: DuStatus) -> Result<(), DuError> {
    match status {
        DuStatus::Ok => Ok(()),
        DuStatus::ErrIo => Err(DuError::Io(String::new())),
        DuStatus::ErrInvalid => Err(DuError::Invalid(String::new())),
        DuStatus::ErrNull => Err(DuError::Null),
        DuStatus::ErrUsd => Err(DuError::Usd(String::new())),
        DuStatus::ErrVulkan => Err(DuError::Vulkan(String::new())),
    }
}
