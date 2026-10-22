pub mod modal;

/// Slint's platform errors are not `windows::core::Error`s. One conversion keeps
/// the host code from repeating the HRESULT.
pub fn platform_error(error: slint::PlatformError) -> windows::core::Error {
    windows::core::Error::new(
        windows::core::HRESULT(0x8000_4005u32 as i32),
        error.to_string(),
    )
}
