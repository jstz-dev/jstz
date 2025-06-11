#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use deno_core::*;
use deno_error::{JsError, JsErrorBox};
use deno_fetch_base::FetchHandler;

use crate::ext::NotSupported;

#[allow(non_camel_case_types)]
pub type jstz_fetch = deno_fetch_base::deno_fetch;

const NOT_SUPPORTED_ERROR: NotSupported = NotSupported { name: "fetch" };

pub struct NotSupportedFetch;

impl FetchHandler for NotSupportedFetch {
    type CreateHttpClientArgs = ();

    type FetchError = NotSupported;

    type Options = ();

    fn fetch(
        state: &mut deno_core::OpState,
        method: ByteString,
        url: String,
        headers: Vec<(ByteString, ByteString)>,
        client_rid: Option<u32>,
        has_body: bool,
        data: Option<JsBuffer>,
        resource: Option<ResourceId>,
    ) -> std::result::Result<deno_fetch_base::FetchReturn, Self::FetchError> {
        Err(NOT_SUPPORTED_ERROR)
    }

    async fn fetch_send(
        state: Rc<std::cell::RefCell<deno_core::OpState>>,
        rid: ResourceId,
    ) -> std::result::Result<deno_fetch_base::FetchResponse, Self::FetchError> {
        Err(NOT_SUPPORTED_ERROR)
    }

    fn custom_client(
        state: &mut deno_core::OpState,
        args: Self::CreateHttpClientArgs,
    ) -> std::result::Result<ResourceId, Self::FetchError> {
        Err(NOT_SUPPORTED_ERROR)
    }
}

#[cfg(test)]
mod test {
    use deno_error::JsErrorClass;

    use crate::{init_test_setup, JstzRuntime, JstzRuntimeOptions};

    #[test]
    fn fetch_not_supported() {
        let mut runtime = JstzRuntime::new(JstzRuntimeOptions::default());
        let code = r#"await fetch("https://example.com").then(res => res.text())"#;
        let err = runtime.execute(code).unwrap_err();
        assert_eq!(
            "Error: Uncaught undefined",
            format!("{}: {}", err.get_class(), err.get_message())
        );
    }
}
