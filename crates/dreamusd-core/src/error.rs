use std::ffi::CStr;

use dreamusd_sys::{du_get_last_error, DuStatus};

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
    let message = last_error_message();
    match status {
        DuStatus::Ok => Ok(()),
        DuStatus::ErrIo => Err(DuError::Io(message)),
        DuStatus::ErrInvalid => Err(DuError::Invalid(message)),
        DuStatus::ErrNull => Err(DuError::Null),
        DuStatus::ErrUsd => Err(DuError::Usd(message)),
        DuStatus::ErrVulkan => Err(DuError::Vulkan(message)),
    }
}

fn last_error_message() -> String {
    let mut message = std::ptr::null();
    let status = unsafe { du_get_last_error(&mut message) };
    if status == DuStatus::Ok && !message.is_null() {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    }
}
