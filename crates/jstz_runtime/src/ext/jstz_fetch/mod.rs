#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use deno_core::*;
use deno_fetch_ext::FetchHandler;

#[allow(non_camel_case_types)]
type jstz_fetch = deno_fetch_ext::deno_fetch;

pub struct JstzFetchHandler;

impl FetchHandler for JstzFetchHandler {
    type CreateHttpClientArgs = ();

    type FetchError = FetchError;

    type Options = ();

    fn fetch(
        state: &mut OpState,
        method: ByteString,
        url: String,
        headers: Vec<(ByteString, ByteString)>,
        client_rid: Option<u32>,
        has_body: bool,
        data: Option<JsBuffer>,
        resource: Option<ResourceId>,
    ) -> Result<deno_fetch_ext::FetchReturn, Self::FetchError> {
        todo!()
    }

    async fn fetch_send(
        state: Rc<RefCell<OpState>>,
        rid: ResourceId,
    ) -> Result<deno_fetch_ext::FetchResponse, Self::FetchError> {
        Err(FetchError::Unimplemented)
    }

    fn custom_client(
        state: &mut deno_core::OpState,
        args: Self::CreateHttpClientArgs,
    ) -> Result<ResourceId, Self::FetchError> {
        todo!()
    }
}

#[derive(Debug, ::thiserror::Error, deno_error::JsError)]
pub enum FetchError {
    #[class(type)]
    #[error("unimplemented")]
    Unimplemented,
}
