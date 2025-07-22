#![allow(unused)]
use deno_core::ByteString;
use futures::{
    channel::oneshot::{channel, Receiver, Sender},
    future::UnwrapOrElse,
};
use jstz_core::{
    host::HostRuntime,
    kv::{Storage, Transaction},
};
use jstz_crypto::public_key::PublicKey;
use std::{collections::BTreeMap, fmt::Display, ops::Deref};
use tezos_smart_rollup::storage::path::{concat, OwnedPath};

use super::{OracleRequest, RequestId, UserAddress};
use crate::{
    context::account::Account,
    event::{EventError, EventPublisher},
    runtime::v2::{
        fetch::http::{Request, Response},
        protocol_context::PROTOCOL_CONTEXT,
    },
    storage::{ORACLE_PUBLIC_KEY_PATH, ORACLE_REQUESTS_PATH},
    BlockLevel, Gas,
};

static X_JSTZ_ORACLE_GAS_LIMIT: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-oracle-gas-limit"));

// FIXME(https://linear.app/tezos/issue/JSTZ-744/make-ttl-configurable)
const ORACLE_REQUEST_TTL: u64 = 20;

#[derive(Debug)]
pub struct Oracle {
    /// Oracle's public key
    public_key: PublicKey,
    /// Holds cached metadata that is checked often
    ///
    /// Notes on timeout: The relationship between request id and timeout is such that
    /// the timeout of the next rid will be at minimum equal to the timeout of the latest
    /// rid. In addition, since rids are created through an incrementing counter, the
    /// order of an rid is equivalent to timeout order. This means we can rely on `first_entry`
    /// of BTreeMap to get the next timeout value (and efficiently delete it) while also having
    /// an efficient way to delete requests by rid
    active_requests: BTreeMap<RequestId, RequestMetadata>,
    /// Next request id
    next_request_id: RequestId,
    config: OracleConfig,
}

impl Oracle {
    /// Instantiates the oracle
    ///
    /// [`ORACLE_PUBLIC_KEY_PATH`] must be set before this function is called. This function
    /// should only be called once throughout the lifetime of Jstz
    pub fn new(rt: &impl HostRuntime, config: Option<OracleConfig>) -> Result<Self> {
        let public_key = Storage::get::<PublicKey>(rt, &ORACLE_PUBLIC_KEY_PATH)
            .map_err(|e| OracleError::V1Error(e.to_string()))?
            .ok_or(OracleError::PublicKeyNotFound)?;
        Ok(Self {
            public_key,
            active_requests: Default::default(),
            next_request_id: 0,
            config: config.unwrap_or_default(),
        })
    }

    /// Sends an Oracle request by publishing an [`OracleRequest`] event
    ///
    /// # Gas
    /// This function will check that the user account meets the minimum gas limit then deducts a gas bond from the
    /// user account directly from storage and the transaction. If [`X_JSTZ_ORACLE_GAS_LIMIT`] header exists, it
    /// will be used as the limit instead. Note that [`X_JSTZ_ORACLE_GAS_LIMIT`] must be gte to the minimum gas
    /// limit. At a later point, the bond will be return back to the user account sub any gas used for processing
    /// the response before resumption.
    pub fn send_request(
        &mut self,
        rt: &mut impl HostRuntime,
        tx: &mut Transaction,
        caller: &UserAddress,
        request: Request,
    ) -> Result<Receiver<Response>> {
        let gas_limit = self.calculate_gas_limit(&request)?;
        // TODO(https://linear.app/tezos/issue/JSTZ-735/fix-transaction-bond-issue)
        // Deduce balance for bond
        let request_id = self.next_request_id;
        let current_level = PROTOCOL_CONTEXT
            .get()
            .expect("Protocol context should be initialized")
            .current_level();
        let timeout = current_level + ORACLE_REQUEST_TTL;
        let oracle_request = OracleRequest {
            id: request_id,
            caller: caller.clone(),
            gas_limit,
            timeout,
            request: request,
        };
        let (sender, rx) = channel();
        if self.active_requests.contains_key(&request_id) {
            // protocol error
            return Err(OracleError::BadState("Sender should not yet exist!"));
        }

        // Checks have passed, we can do state updates
        self.incr_request_id();
        OracleRequestStorage::insert(rt, &oracle_request);
        self.active_requests
            .insert(request_id, RequestMetadata { sender, timeout });
        EventPublisher::publish_event(rt, &oracle_request)?;
        Ok(rx)
    }

    pub fn respond(
        &mut self,
        host: &mut impl HostRuntime,
        request_id: RequestId,
        response: Response,
    ) -> Result<()> {
        let (oracle_request, request_metadata) = self.remove(host, &request_id)?;
        if request_metadata.sender.send(response).is_err() {
            return Err(OracleError::ConnectionClosed);
        }
        // TODO(https://linear.app/tezos/issue/JSTZ-735/fix-oracle-bond-issue)
        // Recredit the account
        Ok(())
    }

    /// Removes and returns the OracleRequest from starage and Sender from internal
    /// senders map
    pub fn remove(
        &mut self,
        host: &mut impl HostRuntime,
        request_id: &RequestId,
    ) -> Result<(OracleRequest, RequestMetadata)> {
        let request_metadata = self
            .active_requests
            .remove(&request_id)
            .ok_or_else(|| OracleError::RequestDoesNotExist)?;
        let oracle_request = OracleRequestStorage::get(host, &request_id).unwrap();
        OracleRequestStorage::delete(host, &request_id);
        Ok((oracle_request, request_metadata))
    }

    // Increments and returns the previous [`next_request_id`]
    fn incr_request_id(&mut self) -> RequestId {
        let curr = self.next_request_id;
        self.next_request_id += 1;
        curr
    }

    // Check `X-JSTZ-ORACLE-GAS-LIMIT` is `PROTOCOL_GAS + ORACLE_FEE + SPAM_PREVENTION`
    fn calculate_gas_limit(&self, request: &Request) -> Result<Gas> {
        // TODO(https://linear.app/tezos/issue/JSTZ-673/deduct-gas-from-top-level-gas-limit)
        // Deduct from top level gas limit
        let minimum_gas_limit = self.config.gas.protocol_fee
            + self.config.gas.oracle_fee
            + self.config.gas.spam_prevention;
        let gas_limit: u64 = request
            .headers
            .iter()
            .find(|(key, value)| key.eq_ignore_ascii_case(&X_JSTZ_ORACLE_GAS_LIMIT))
            .and_then(|(key, value)| match String::from_utf8(value.to_vec()) {
                Ok(s) => s.parse::<u64>().ok(),
                Err(_) => None,
            })
            .unwrap_or_else(|| minimum_gas_limit);
        if gas_limit < minimum_gas_limit {
            Err(OracleError::GasLimitTooLow(minimum_gas_limit))?
        }
        Ok(gas_limit)
    }

    /// Triggers the GC for timed out requests
    pub fn gc_timeout_requests(&mut self, host: &mut impl HostRuntime) {
        let current_level = PROTOCOL_CONTEXT
            .get()
            .expect("Protocol context should be initialized")
            .current_level();
        while let Some(head) = self.active_requests.first_entry() {
            if current_level < head.get().timeout {
                break;
            }
            {
                // Sender will send cancellation when dropped
                head.remove_entry();
            }
        }
    }

    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }
}

#[derive(Debug, Default)]
pub struct OracleConfig {
    gas: GasParams,
}

#[derive(Clone, Debug, Default)]
struct GasParams {
    protocol_fee: Gas,
    oracle_fee: Gas,
    spam_prevention: Gas,
}

#[derive(Debug)]
pub struct RequestMetadata {
    sender: Sender<Response>,
    timeout: BlockLevel,
}

struct OracleRequestStorage;

impl OracleRequestStorage {
    fn path(request_id: &RequestId) -> OwnedPath {
        concat(
            &ORACLE_REQUESTS_PATH,
            &OwnedPath::try_from(format!("/{}", request_id.to_string())).unwrap(),
        )
        .unwrap()
    }

    fn get(rt: &impl HostRuntime, request_id: &RequestId) -> Option<OracleRequest> {
        let path = OracleRequestStorage::path(request_id);
        Storage::get::<OracleRequest>(rt, &path).unwrap()
    }

    fn insert(rt: &mut impl HostRuntime, request: &OracleRequest) {
        let path = OracleRequestStorage::path(&request.id);
        Storage::insert(rt, &path, request).unwrap();
    }

    fn delete(rt: &mut impl HostRuntime, request_id: &RequestId) {
        let path = OracleRequestStorage::path(request_id);

        Storage::remove(rt, &path).unwrap()
    }
}

type Result<T> = std::result::Result<T, OracleError>;
#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class("OracleError")]
pub enum OracleError {
    #[error("Oracle signer public key not found at '{ORACLE_PUBLIC_KEY_PATH}'")]
    PublicKeyNotFound,

    #[error("{0}")]
    V1Error(String),

    #[error("Oracle gas limit too low. Must be at least {0} mutez at this time")]
    GasLimitTooLow(Gas),

    #[error(transparent)]
    EventError(#[from] EventError),

    #[error("{0}")]
    BadState(&'static str),

    #[error("Request Id does not exist or has expired")]
    RequestDoesNotExist,

    #[error("Connection closed by client")]
    ConnectionClosed,
}

impl From<crate::error::Error> for OracleError {
    fn from(value: crate::error::Error) -> Self {
        Self::V1Error(value.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::context::account::Account;
    use crate::event::decode_line;
    use crate::runtime::v2::fetch::http::{Body, Request, Response};
    use crate::runtime::v2::oracle::UserAddress;
    use crate::runtime::v2::protocol_context::ProtocolContext;
    use crate::tests::DebugLogSink;
    use jstz_core::kv::Storage;
    use jstz_crypto::{hash::Hash, public_key::PublicKey};
    use serde_json::json;
    use tezos_smart_rollup_mock::MockHost;

    fn setup_host_with_pk(pk: &PublicKey, sink: Option<DebugLogSink>) -> MockHost {
        let mut host = MockHost::default();
        Storage::insert(&mut host, &ORACLE_PUBLIC_KEY_PATH, pk).unwrap();
        ProtocolContext::init_global(&mut host, 0).unwrap();
        if let Some(sink) = sink {
            host.set_debug_handler(sink);
        }
        host
    }

    fn setup_with_user_and_gas_params(
        user_balance: u64,
        gas_params: &GasParams,
    ) -> (Oracle, MockHost, DebugLogSink, UserAddress) {
        // Setup
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let sink = DebugLogSink::new();
        let mut host = setup_host_with_pk(&pk, Some(sink.clone()));
        let mut tx = Transaction::default();
        tx.begin();
        let caller = UserAddress::digest(&[1u8; 20]).unwrap();
        // Give the caller enough balance for gas
        Account::set_balance(&mut host, &mut tx, &caller, user_balance).unwrap();
        tx.commit(&mut host);

        let mut oracle = Oracle::new(
            &host,
            Some(OracleConfig {
                gas: gas_params.clone(),
            }),
        )
        .unwrap();

        (oracle, host, sink, caller)
    }

    #[test]
    fn oracle_new_success() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let host = setup_host_with_pk(&pk, None);
        let oracle = Oracle::new(&host, None).expect("should succeed");
        assert_eq!(oracle.public_key, pk);
        assert_eq!(oracle.next_request_id, 0);
        assert!(oracle.active_requests.is_empty());
    }

    #[test]
    fn oracle_new_missing_public_key() {
        let host = MockHost::default();
        let err = Oracle::new(&host, None).unwrap_err();
        assert!(matches!(err, OracleError::PublicKeyNotFound));
    }

    #[test]
    fn oracle_incr_request_id_increments() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let host = setup_host_with_pk(&pk, None);
        let mut oracle = Oracle::new(&host, None).unwrap();
        assert_eq!(oracle.incr_request_id(), 0);
        assert_eq!(oracle.incr_request_id(), 1);
        assert_eq!(oracle.incr_request_id(), 2);
    }

    #[tokio::test]
    async fn send_request_success() {
        let gas_params = GasParams {
            protocol_fee: 1_000,
            oracle_fee: 340,
            spam_prevention: 120,
        };

        // Setup
        let (mut oracle, mut host, sink, caller) =
            setup_with_user_and_gas_params(1_000_000, &gas_params);

        let mut tx = Transaction::default();
        tx.begin();

        let minimal_gas =
            gas_params.oracle_fee + gas_params.protocol_fee + gas_params.spam_prevention;

        // Prepare a request
        let request = Request {
            method: "GET".into(),
            url: "https://example.com".parse().unwrap(),
            headers: vec![],
            body: None,
        };

        // Call send_request
        let rx1 = oracle
            .send_request(&mut host, &mut tx, &caller, request.clone())
            .expect("send_request should succeed");

        // Check Oracle state: next_request_id incremented, sender inserted
        assert_eq!(oracle.next_request_id, 1);
        assert_eq!(oracle.active_requests.len(), 1);
        assert!(oracle.active_requests.contains_key(&0));
        let (_, value) = oracle.active_requests.first_key_value().unwrap();
        assert_eq!(value.timeout, ORACLE_REQUEST_TTL);

        // Check OracleRequest is stored
        let stored =
            OracleRequestStorage::get(&host, &0).expect("OracleRequest should be stored");
        assert_eq!(0, stored.id);
        assert_eq!(caller, stored.caller);
        assert_eq!(request.clone(), stored.request);
        assert_eq!(ORACLE_REQUEST_TTL, stored.timeout);
        assert_eq!(minimal_gas, stored.gas_limit);

        // TODO(Deduct balances)
        // See calculate_gas_limi
        // let balance = Account::balance(&host, &mut tx, &caller).unwrap();
        // assert_eq!(1_000_000 - minimal_gas, balance);

        let line = sink.lines().first().unwrap().clone();
        assert_eq!(stored, decode_line(&line).unwrap());

        // Second requst but this time with X_JSTZ_ORACLE_GAS_LIMIT header
        let headers = vec![("x-jstz-oracle-gas-limit".into(), "3500".into())];
        let request2 = Request { headers, ..request };

        let rx2 = oracle
            .send_request(&mut host, &mut tx, &caller, request2.clone())
            .expect("send_request should succeed");

        assert_eq!(oracle.next_request_id, 2);
        assert_eq!(oracle.active_requests.len(), 2);
        assert!(oracle.active_requests.contains_key(&1));

        let stored2 =
            OracleRequestStorage::get(&host, &1).expect("OracleRequest should be stored");
        assert_eq!(1, stored2.id);
        assert_eq!(caller, stored.caller);
        assert_eq!(request2.clone(), stored2.request);
        assert_eq!(ORACLE_REQUEST_TTL, stored2.timeout);
        assert_eq!(3500, stored2.gas_limit);

        // TODO(Deduct balances)
        // See calculate_gas_limit
        // let balance = Account::balance(&host, &mut tx, &caller).unwrap();

        // // Expected is initial - request 1 gas - request 2 gas
        // assert_eq!(1_000_000 - minimal_gas - 3500, balance);

        let line = sink.lines().iter().nth(1).unwrap().clone();
        assert_eq!(stored2, decode_line(&line).unwrap());

        let response = Response {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: Body::zero_capacity(),
        };
        oracle.respond(&mut host, 1, response.clone()).unwrap();
        let recv_response = rx2.await.unwrap();
        assert_eq!(response, recv_response);

        oracle.respond(&mut host, 0, response.clone()).unwrap();
        let recv_response = rx1.await.unwrap();
        assert_eq!(response, recv_response);
    }

    #[test]
    fn send_request_below_minimal_gas_fails() {
        let gas_params = GasParams {
            protocol_fee: 1_000,
            oracle_fee: 340,
            spam_prevention: 120,
        };

        let (mut oracle, mut host, request, caller) =
            setup_with_user_and_gas_params(1_000_000, &gas_params);
        let mut tx = Transaction::default();
        tx.begin();

        // Prepare a request
        let request = Request {
            method: "POST".into(),
            url: "https://example.com".parse().unwrap(),
            headers: vec![("x-jstz-oracle-gas-limit".into(), "100".into())],
            body: Some(
                serde_json::to_vec(&json!({ "message": "hello" }))
                    .unwrap()
                    .as_slice()
                    .into(),
            ),
        };
        let error = oracle
            .send_request(&mut host, &mut tx, &caller, request.clone())
            .unwrap_err();

        assert_eq!(
            "Oracle gas limit too low. Must be at least 1460 mutez at this time",
            error.to_string()
        );
    }

    #[test]
    fn respond_missing_request() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let mut host = setup_host_with_pk(&pk, None);
        let mut oracle = Oracle::new(&host, None).unwrap();
        let response = Response {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: Body::zero_capacity(),
        };
        let err = oracle.respond(&mut host, 10, response).unwrap_err();
        assert!(matches!(err, OracleError::RequestDoesNotExist))
    }

    #[test]
    fn respond_dropped_receiver() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let mut host = setup_host_with_pk(&pk, None);
        let mut oracle = Oracle::new(&host, None).unwrap();
        let mut tx = Transaction::default();
        tx.begin();
        let caller = UserAddress::digest(&[1u8; 20]).unwrap();
        Account::add_balance(&mut host, &mut tx, &caller, 100_000);
        tx.commit(&mut host);
        tx.begin();
        let rx = oracle
            .send_request(
                &mut host,
                &mut tx,
                &caller,
                Request {
                    method: "GET".into(),
                    url: "http://example.com".parse().unwrap(),
                    headers: vec![],
                    body: Some(Body::zero_capacity()),
                },
            )
            .unwrap();
    }

    #[test]
    fn remove_from_oracle_storage_and_sender() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let mut host = setup_host_with_pk(&pk, None);
        let mut oracle = Oracle::new(&host, None).unwrap();
        let mut tx = Transaction::default();
        tx.begin();
        let caller = UserAddress::digest(&[1u8; 20]).unwrap();
        Account::add_balance(&mut host, &mut tx, &caller, 100_000);
        tx.commit(&mut host);
        tx.begin();
        oracle
            .send_request(
                &mut host,
                &mut tx,
                &caller,
                Request {
                    method: "GET".into(),
                    url: "http://example.com".parse().unwrap(),
                    headers: vec![],
                    body: Some(Body::zero_capacity()),
                },
            )
            .unwrap();
        oracle.remove(&mut host, &0).unwrap();
        assert_eq!(false, oracle.active_requests.contains_key(&0));
        assert_eq!(None, OracleRequestStorage::get(&mut host, &0))
    }

    #[test]
    fn test_garbage_collect_timeout_requests() {
        let pk = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )
        .unwrap();
        let mut host = setup_host_with_pk(&pk, None);
        let mut oracle = Oracle::new(&host, None).unwrap();
        let mut tx = Transaction::default();
        tx.begin();
        let caller = UserAddress::digest(&[1u8; 20]).unwrap();
        Account::add_balance(&mut host, &mut tx, &caller, 100_000);
        tx.commit(&mut host);
        tx.begin();
        let req = Request {
            method: "GET".into(),
            url: "http://example.com".parse().unwrap(),
            headers: vec![],
            body: Some(Body::zero_capacity()),
        };
        PROTOCOL_CONTEXT.get().unwrap().set_level(1);
        // next 2 requests will expire at level 21
        oracle
            .send_request(&mut host, &mut tx, &caller, req.clone())
            .unwrap();
        oracle
            .send_request(&mut host, &mut tx, &caller, req.clone())
            .unwrap();
        // next 2 requests will expire at level 26
        PROTOCOL_CONTEXT.get().unwrap().set_level(5);
        oracle
            .send_request(&mut host, &mut tx, &caller, req)
            .unwrap();

        assert_eq!(oracle.active_requests.len(), 3);
        oracle.gc_timeout_requests(&mut host);
        assert_eq!(oracle.active_requests.len(), 3);

        PROTOCOL_CONTEXT.get().unwrap().set_level(21);
        oracle.gc_timeout_requests(&mut host);
        assert_eq!(oracle.active_requests.len(), 1);
        assert!(oracle.active_requests.contains_key(&2));

        PROTOCOL_CONTEXT.get().unwrap().set_level(26);
        oracle.gc_timeout_requests(&mut host);
        assert_eq!(oracle.active_requests.len(), 0);
    }
}
