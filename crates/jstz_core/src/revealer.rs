use crate::host::HostError;

/// A type that can load the result of `request` into `response` from the host.
pub trait Revealer {
    /// # Safety
    ///
    /// The host has to handle the reveal request and response accordingly.
    unsafe fn reveal(
        request: &[u8],
        response: &mut [u8],
    ) -> std::result::Result<usize, HostError>;
}

impl Revealer for () {
    unsafe fn reveal(_: &[u8], _: &mut [u8]) -> std::result::Result<usize, HostError> {
        unimplemented!()
    }
}
