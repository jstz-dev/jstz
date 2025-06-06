use crate::runtime::v2::fetch::error::{FetchError, Result};
use crate::runtime::v2::fetch::http::*;

use deno_core::{resolve_import, v8, ByteString, StaticModuleLoader};
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_runtime::sys::{
    FromV8, Headers as JsHeaders, Request as JsRequest, RequestInit as JsRequestInit,
    Response as JsResponse, ToV8,
};
use jstz_runtime::JstzRuntime;
use jstz_runtime::{JstzRuntimeOptions, ProtocolContext};
use std::rc::Rc;
use url::Url;

use crate::context::account::{Account, Address};
use crate::runtime::v2::fetch::fetch_handler::ProtoFetchHandler;

pub struct HostScript;

impl HostScript {
    pub async fn route(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        from: Address,
        method: ByteString,
        url: Url,
    ) -> Result<Response> {
        let path = url.path();
        if path.starts_with("/balances") {
            return Self::handle_balance(host, tx, from, method, url).await;
        }

        // Return 404 for all other paths
        Ok(Response {
            status: 404,
            status_text: "Not Found".to_string(),
            headers: vec![],
            body: Body::Vector("Not Found".as_bytes().to_vec()),
        })
    }

    pub async fn handle_balance(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        self_address: Address,
        method: ByteString,
        url: Url,
    ) -> Result<Response> {
        if method != "GET".into() {
            return Ok(Response {
                status: 405,
                status_text: "Method Not Allowed".to_string(),
                headers: vec![],
                body: Body::Vector("Only GET method is allowed".as_bytes().to_vec()),
            });
        }

        let path = url.path();
        let address_str = path
            .strip_prefix("/balances/")
            .ok_or_else(|| FetchError::JstzError("Invalid path format".to_string()))?;

        match Self::get_balance(host, tx, address_str, &self_address) {
            Ok(balance) => Ok(Response {
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![],
                body: balance.to_string().into(),
            }),
            Err(e) => Ok(Response {
                status: 400,
                status_text: "Bad Request".to_string(),
                headers: vec![],
                body: e.to_string().into(),
            }),
        }
    }

    fn get_balance(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        address_str: &str,
        self_address: &Address,
    ) -> Result<u64> {
        let target_address = if address_str == "self" {
            self_address.clone().into()
        } else {
            Address::from_base58(address_str)
                .map_err(|e| FetchError::JstzError(e.to_string()))?
        };

        Account::balance(host, tx, &target_address)
            .map_err(|e| FetchError::JstzError(e.to_string()))
    }
}
