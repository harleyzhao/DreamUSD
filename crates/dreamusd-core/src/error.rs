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

/// Check a raw status code and convert non-zero values to an error.
///
/// Status codes:
/// - 0: Ok
/// - 1: ErrIo
/// - 2: ErrInvalid
/// - 3: ErrNull
/// - 4: ErrUsd
/// - 5: ErrVulkan
pub(crate) fn check(status: i32) -> Result<(), DuError> {
    match status {
        0 => Ok(()),
        1 => Err(DuError::Io(String::new())),
        2 => Err(DuError::Invalid(String::new())),
        3 => Err(DuError::Null),
        4 => Err(DuError::Usd(String::new())),
        5 => Err(DuError::Vulkan(String::new())),
        other => Err(DuError::Invalid(format!("unknown status code: {other}"))),
    }
}
