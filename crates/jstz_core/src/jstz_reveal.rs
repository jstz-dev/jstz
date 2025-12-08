use crate::error::Result;
use crate::BinEncodable;
use tezos_smart_rollup::host::{HostError, RuntimeError};
use tezos_smart_rollup_constants::riscv::REVEAL_REQUEST_MAX_SIZE;

/// The reveal tag value for JSTZ-specific reveal requests.
pub const JSTZ_REVEAL_TAG: u8 = 0xFF;

#[derive(Debug, thiserror::Error)]
pub enum JstzRevealError {
    #[error("Reveal data size exceeds the maximum limit.")]
    RevealSizeExceedsMaximumLimit,

    #[error("Runtime error during reveal: {0}")]
    Runtime(#[from] RuntimeError),
}

/// Sends a JSTZ-specific reveal request to the host and returns the decoded response.
///
/// # Arguments
///
/// * `request` - The request data to be encoded and sent. Must implement [`BinEncodable`].
///
/// # Returns
///
/// The decoded response of type `Resp`, or an error if:
/// - The encoded request exceeds [`REVEAL_REQUEST_MAX_SIZE`]
/// - The host's reveal call fails
/// - The response cannot be decoded into type `Resp`
///
/// # Safety
///
/// The rollup host must be configured to handle reveal requests with tag value [`JSTZ_REVEAL_TAG`] (0xFF).
pub unsafe fn send_request<Req, Resp>(
    request: &Req,
    #[cfg(test)] mock_reveal: impl tests::MockRevealFn,
) -> Result<Resp>
where
    Req: BinEncodable,
    Resp: BinEncodable,
{
    // Build request: [tag, <encoded request body>]
    let body = request.encode()?;
    let mut encoded = Vec::with_capacity(1 + body.len());
    encoded.extend_from_slice(&[JSTZ_REVEAL_TAG]);
    encoded.extend_from_slice(&body);

    if encoded.len() > REVEAL_REQUEST_MAX_SIZE {
        return Err(JstzRevealError::RevealSizeExceedsMaximumLimit.into());
    }

    let mut response_buf = [0u8; REVEAL_REQUEST_MAX_SIZE];

    let response_len = {
        #[cfg(not(test))]
        unsafe {
            reveal(&encoded, &mut response_buf)
        }

        #[cfg(test)]
        mock_reveal(&encoded, &mut response_buf)
    }?;

    let resp = Resp::decode(&response_buf[..response_len])?;

    Ok(resp)
}

/// Loads the result of a raw reveal request to memory.
/// Both the request and response buffers are limited to [`REVEAL_REQUEST_MAX_SIZE`] bytes.
///
/// # Parameters
///
/// * `request` - The encoded reveal request bytes. The first byte should be the reveal tag.
/// * `response` - A mutable buffer where the reveal response will be written.
///
/// # Returns
///
/// Returns `Result<usize, RuntimeError>` where:
/// * `Ok(len)` - Contains the number of bytes written to the `response` buffer.
/// * `Err(e)` - Contains a [`RuntimeError`] if the reveal operation fails.
///
/// # Safety
///
/// The first value in the request is used as a tag to determine the kind of reveal request,
/// and the kernel host should be able to handle this accordingly.
/// The reveal tag from 0 to 3 is reserved by the rollup host - do not use them.
#[inline]
unsafe fn reveal(
    request: &[u8],
    response: &mut [u8],
) -> std::result::Result<usize, RuntimeError> {
    use tezos_smart_rollup::core_unsafe::rollup_host::RollupHost;
    use tezos_smart_rollup::core_unsafe::smart_rollup_core::SmartRollupCore;

    // technically unsafe, but RollupHost is a ZST so we don't break anything
    let inner_host = RollupHost::new();
    let len = <RollupHost as SmartRollupCore>::reveal(
        &inner_host,
        request.as_ptr(),
        request.len(),
        response.as_mut_ptr(),
        response.len(),
    );

    match HostError::wrap(len) {
        Ok(len) => Ok(len),
        Err(e) => Err(RuntimeError::HostErr(e)),
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use super::*;
    use crate::BinEncodable;
    use bincode::{Decode, Encode};

    pub trait MockRevealFn:
        FnOnce(&[u8], &mut [u8]) -> std::result::Result<usize, RuntimeError>
    {
    }
    impl<F> MockRevealFn for F where
        F: FnOnce(&[u8], &mut [u8]) -> std::result::Result<usize, RuntimeError>
    {
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    pub struct TestData(pub Vec<u8>);

    impl Deref for TestData {
        type Target = [u8];
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[test]
    fn sends_reveal_request() {
        let mock_request = TestData(vec![1, 2, 3]);
        let mock_request_encoded =
            <TestData as BinEncodable>::encode(&mock_request).unwrap();
        let mock_response = TestData(vec![3, 2, 1]);
        let mock_response_encoded =
            <TestData as BinEncodable>::encode(&mock_response).unwrap();

        // Returns the mock response
        let mock_reveal = |request: &[u8], response: &mut [u8]| {
            assert_eq!(request[0], JSTZ_REVEAL_TAG);
            assert_eq!(&request[1..], mock_request_encoded.as_slice());
            let len = mock_response_encoded.len();
            response[..len].copy_from_slice(&mock_response_encoded);
            Ok(len)
        };

        let response =
            unsafe { send_request::<TestData, TestData>(&mock_request, mock_reveal) };

        assert_eq!(response.unwrap(), mock_response);
    }

    #[test]
    fn throws_error_for_invalid_response() {
        let mock_request = TestData(vec![1, 2, 3]);

        // Returns invalid response
        let mock_reveal = |_request: &[u8], response: &mut [u8]| {
            response[..3].copy_from_slice(&[3, 2, 1]);
            Ok(3)
        };

        let response_err =
            unsafe { send_request::<TestData, TestData>(&mock_request, mock_reveal) }
                .unwrap_err();
        assert!(matches!(
            response_err,
            crate::error::Error::SerializationError { description: _ }
        ));
    }

    #[test]
    fn throws_error_for_reveal_size_exceeds_maximum_limit() {
        let mock_request = TestData(vec![1; REVEAL_REQUEST_MAX_SIZE]);
        let mock_request_encoded =
            <TestData as BinEncodable>::encode(&mock_request).unwrap();
        assert!(mock_request_encoded.len() > REVEAL_REQUEST_MAX_SIZE);

        let response_err =
            unsafe { send_request::<TestData, TestData>(&mock_request, |_, _| Ok(0)) }
                .unwrap_err();

        assert!(matches!(
            response_err,
            crate::error::Error::RevealError {
                source: JstzRevealError::RevealSizeExceedsMaximumLimit
            }
        ));
    }
}
