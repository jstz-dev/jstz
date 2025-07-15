#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use deno_core::*;
use deno_error::{JsError, JsErrorBox};
use deno_fetch_base::{deno_fetch, FetchHandler};

use crate::ext::NotSupported;

#[allow(non_camel_case_types)]
pub type jstz_fetch = deno_fetch_base::deno_fetch;

pub trait FetchHandlerOptions: FetchHandler {
    fn options() -> Self::Options;
}

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

impl FetchHandlerOptions for NotSupportedFetch {
    fn options() -> Self::Options {}
}

pub trait FetchAPI: FetchHandler + FetchHandlerOptions {}

impl<T: FetchHandler + FetchHandlerOptions> FetchAPI for T {}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use deno_core::StaticModuleLoader;
    use deno_error::JsErrorClass;
    use jstz_utils::test_util::TOKIO;

    use crate::{init_test_setup, JstzRuntime, JstzRuntimeOptions};

    #[test]
    fn fetch_not_supported() {
        TOKIO.block_on(async {
            let code = r#"export default async () => await fetch("https://example.com").then(res => res.text())"#;
            let specifier = deno_core::resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
            let loader = StaticModuleLoader::with(specifier.clone(), code);
            let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
                module_loader: Rc::new(loader),
                ..Default::default()
            });
            let id = runtime.execute_main_module(&specifier).await.unwrap();
            let err = runtime.call_default_handler(id, &[]).await.unwrap_err();
            assert_eq!(
                "Error: Uncaught (in promise) undefined",
                format!("{}: {}", err.get_class(), err.get_message())
            );
        });
    }
}
