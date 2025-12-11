use crate::error::Result;
use crate::{BinEncodable, Revealer};
use tezos_smart_rollup::host::RuntimeError;
use tezos_smart_rollup_constants::riscv::REVEAL_REQUEST_MAX_SIZE;

/// The reveal tag value for JSTZ-specific reveal requests.
pub const JSTZ_REVEAL_TAG: u8 = 0xFF;

#[derive(Debug, thiserror::Error)]
pub enum JstzRevealError {
    #[error("Jstz reveal data size exceeds the maximum limit.")]
    RevealSizeExceedsMaximumLimit,
}

/// The host revealer that loads the result of a raw reveal request to memory.
/// `request` and `response` should not exceed `REVEAL_REQUEST_MAX_SIZE`.
///
/// # Safety
///
/// The first value in the request is used as a tag to determine the kind of reveal request,
/// and the kernel host should be able to handle this accordingly.
/// The reveal tag from 0 to 3 is reserved by the rollup host - do not use them.
pub struct HostRevealer;

impl Revealer for HostRevealer {
    unsafe fn reveal(
        request: &[u8],
        response: &mut [u8],
    ) -> std::result::Result<usize, RuntimeError> {
        reveal(request, response)
    }
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
/// - The reveal call fails
/// - The response cannot be decoded into type `Resp`
pub fn send_request<Rv, Req, Resp>(request: &Req) -> Result<Resp>
where
    Rv: Revealer,
    Req: BinEncodable,
    Resp: BinEncodable,
{
    // Build request: [tag, <encoded request body>]
    let body = request.encode()?;
    let mut encoded = Vec::with_capacity(1 + body.len());
    encoded.push(JSTZ_REVEAL_TAG);
    encoded.extend_from_slice(&body);

    // TODO: support reveal larger than `REVEAL_REQUEST_MAX_SIZE`
    // https://linear.app/tezos/issue/JSTZ-1045/support-reveal-larger-than-4kb
    if encoded.len() > REVEAL_REQUEST_MAX_SIZE {
        return Err(JstzRevealError::RevealSizeExceedsMaximumLimit.into());
    }

    let mut response_buf = [0u8; REVEAL_REQUEST_MAX_SIZE];

    let response_len = unsafe { Rv::reveal(&encoded, &mut response_buf)? };

    Resp::decode(&response_buf[..response_len])
}

#[allow(dead_code)]
#[inline]
unsafe fn reveal(
    request: &[u8],
    response: &mut [u8],
) -> std::result::Result<usize, RuntimeError> {
    use tezos_smart_rollup::core_unsafe::rollup_host::RollupHost;
    use tezos_smart_rollup::core_unsafe::smart_rollup_core::SmartRollupCore;
    use tezos_smart_rollup::host::HostError;

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
    use tezos_smart_rollup::host::HostError;

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    pub struct TestData(pub Vec<u8>);

    impl Deref for TestData {
        type Target = [u8];
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    fn mock_request() -> TestData {
        TestData(vec![1, 2, 3])
    }

    fn mock_response() -> TestData {
        TestData(vec![3, 2, 1])
    }

    #[test]
    fn sends_reveal_request() {
        struct MockRevealer;
        impl Revealer for MockRevealer {
            unsafe fn reveal(
                request: &[u8],
                response: &mut [u8],
            ) -> std::result::Result<usize, RuntimeError> {
                let mock_request_encoded =
                    <TestData as BinEncodable>::encode(&mock_request()).unwrap();
                let mock_response_encoded =
                    <TestData as BinEncodable>::encode(&mock_response()).unwrap();

                assert_eq!(request[0], JSTZ_REVEAL_TAG);
                assert_eq!(&request[1..], mock_request_encoded.as_slice());
                let len = mock_response_encoded.len();
                response[..len].copy_from_slice(&mock_response_encoded);
                Ok(len)
            }
        }

        let request = mock_request();
        let expected_response = mock_response();

        let actual_response = send_request::<MockRevealer, TestData, TestData>(&request);

        assert_eq!(expected_response, actual_response.unwrap());
    }

    #[test]
    fn throws_error_for_invalid_response() {
        struct MockRevealer;
        impl Revealer for MockRevealer {
            unsafe fn reveal(
                _request: &[u8],
                response: &mut [u8],
            ) -> std::result::Result<usize, RuntimeError> {
                response[..3].copy_from_slice(&[3, 2, 1]);
                Ok(3)
            }
        }

        let mock_request = TestData(vec![1, 2, 3]);

        let response_err =
            send_request::<MockRevealer, TestData, TestData>(&mock_request).unwrap_err();

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
            send_request::<(), TestData, TestData>(&mock_request).unwrap_err();

        assert!(matches!(
            response_err,
            crate::error::Error::JstzRevealError {
                source: JstzRevealError::RevealSizeExceedsMaximumLimit
            }
        ));
    }

    #[test]
    fn propagates_runtime_error() {
        let mock_request = TestData(vec![]);

        struct MockRevealer;
        impl Revealer for MockRevealer {
            unsafe fn reveal(
                _: &[u8],
                _: &mut [u8],
            ) -> std::result::Result<usize, RuntimeError> {
                Err(RuntimeError::HostErr(HostError::InputOutputTooLarge))
            }
        }

        let response_err =
            send_request::<MockRevealer, TestData, TestData>(&mock_request).unwrap_err();

        assert!(matches!(
            response_err,
            crate::error::Error::HostError {
                source: RuntimeError::HostErr(HostError::InputOutputTooLarge)
            }
        ));
    }
}
