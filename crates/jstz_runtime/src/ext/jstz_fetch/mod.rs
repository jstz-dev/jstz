#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use deno_core::*;
use deno_error::{JsError, JsErrorBox};
use deno_fetch_base::FetchHandler;

use crate::ProtocolContext;

#[allow(non_camel_case_types)]
pub type jstz_fetch = deno_fetch_base::deno_fetch;

pub struct NotSupportedFetch;

impl FetchHandler for NotSupportedFetch {
    type CreateHttpClientArgs = ();

    type FetchError = NotSupportedFetchError;

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
        Err(NotSupportedFetchError)
    }

    async fn fetch_send(
        state: Rc<std::cell::RefCell<deno_core::OpState>>,
        rid: ResourceId,
    ) -> std::result::Result<deno_fetch_base::FetchResponse, Self::FetchError> {
        Err(NotSupportedFetchError)
    }

    fn custom_client(
        state: &mut deno_core::OpState,
        args: Self::CreateHttpClientArgs,
    ) -> std::result::Result<ResourceId, Self::FetchError> {
        Err(NotSupportedFetchError)
    }
}

#[derive(Debug, ::thiserror::Error, JsError)]
#[class(type)]
#[error("fetch() is not supported")]
pub struct NotSupportedFetchError;
