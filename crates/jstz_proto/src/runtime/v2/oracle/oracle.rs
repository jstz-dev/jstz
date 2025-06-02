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
use std::{collections::BTreeMap, fmt::Display};
use tezos_smart_rollup::storage::path::{concat, OwnedPath};

use super::{OracleRequest, RequestId, UserAddress};
use crate::{
    context::account::Account,
    event::{Event, EventError, EventPublisher},
    runtime::v2::fetch::http::{Request, Response},
    storage::{ORACLE_PUBLIC_KEY_PATH, ORACLE_REQUESTS_PATH},
    BlockLevel, Gas,
};

static X_JSTZ_ORACLE_GAS_LIMIT: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-oracle-gas-limit"));

#[derive(Debug)]
pub struct Configuration {
    gas: GasParams,
}

#[derive(Debug, Default)]
struct GasParams {
    protocol_fee: Gas,
    oracle_fee: Gas,
    spam_prevention: Gas,
}

#[derive(Debug)]
pub struct Oracle {
    /// Oracle's public key
    public_key: PublicKey,
    /// Sender channels for in-flight requests
    senders: BTreeMap<RequestId, Sender<Response>>,
    /// Next request id
    next_request_id: RequestId,
    config: Configuration,
    publisher: EventPublisher,
}

impl Oracle {
    /// Instantiates the oracle
    ///
    /// [`ORACLE_PUBLIC_KEY_PATH`] must be set before this function is called. This function
    /// should only be called once throughout the lifetime of Jstz
    pub fn new(rt: &impl HostRuntime) -> Result<Self> {
        let public_key = Storage::get::<PublicKey>(rt, &ORACLE_PUBLIC_KEY_PATH)
            .map_err(|e| OracleError::V1Error(e.to_string()))?
            .ok_or(OracleError::PublicKeyNotFound)?;
        Ok(Self {
            public_key,
            senders: Default::default(),
            next_request_id: 0,
            config: Configuration {
                gas: Default::default(),
            },
            publisher: EventPublisher::default(),
        })
    }

    /// Initiates the Oracle fetch request
    pub fn fetch(
        &mut self,
        rt: &mut impl HostRuntime,
        tx: &mut Transaction,
        caller: &UserAddress,
        request: Request,
    ) -> Result<Receiver<Response>> {
        let gas_limit = self.calculate_gas_limit(&request)?;
        let prepared_deduction =
            Account::prepare_eager_sub_balance(rt, tx, caller, gas_limit)?;
        let request_id = self.next_request_id;

        let oracle_request = OracleRequest {
            id: self.incr_request_id(),
            caller: caller.clone(),
            gas_limit,
            timeout: 0,
            request: request,
        };

        let (sender, rx) = channel();
        assert!(!self.senders.contains_key(&request_id));

        // Checks have passed, we can do state updates
        prepared_deduction.apply();
        OracleRequestStorage::insert(rt, &oracle_request);
        self.senders.insert(request_id, sender);
        self.publisher.publish_event(rt, &oracle_request)?;
        Ok(rx)
    }

    // Increments and returns the previous [`next_request_id`]
    fn incr_request_id(&mut self) -> RequestId {
        let curr = self.next_request_id;
        self.next_request_id += 1;
        curr
    }

    // Check `X-JSTZ-ORACLE-GAS-LIMIT` is gte `PROTOCOL_GAS + ORACLE_FEE + SPAM_PREVENTION`
    fn calculate_gas_limit(&self, request: &Request) -> Result<Gas> {
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
}

struct OracleRequestStorage;

impl OracleRequestStorage {
    fn path(request_id: &RequestId) -> OwnedPath {
        concat(
            &ORACLE_REQUESTS_PATH,
            &OwnedPath::try_from(request_id.to_string()).unwrap(),
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
#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    #[error("Oracle signer public key not found at '{ORACLE_PUBLIC_KEY_PATH}'")]
    PublicKeyNotFound,

    #[error("{0}")]
    V1Error(String),

    #[error("Oracle gas limit too low. Must be at least {0} mutez at this time")]
    GasLimitTooLow(Gas),

    #[error(transparent)]
    EventError(#[from] EventError),
}

impl From<crate::error::Error> for OracleError {
    fn from(value: crate::error::Error) -> Self {
        Self::V1Error(value.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_core::kv::Storage;
    use jstz_crypto::public_key::PublicKey;
    use tezos_smart_rollup_mock::MockHost;

    fn setup_host_with_pk(pk: &PublicKey) -> MockHost {
        let mut host = MockHost::default();
        Storage::insert(&mut host, &ORACLE_PUBLIC_KEY_PATH, pk).unwrap();
        host
    }

    #[test]
    fn oracle_new_success() {
        let pk = PublicKey::from_base58(
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )
        .unwrap();
        let host = setup_host_with_pk(&pk);
        let oracle = Oracle::new(&host).expect("should succeed");
        assert_eq!(oracle.public_key, pk);
        assert_eq!(oracle.next_request_id, 0);
        assert!(oracle.senders.is_empty());
    }

    #[test]
    fn oracle_new_missing_public_key() {
        let host = MockHost::default();
        let err = Oracle::new(&host).unwrap_err();
        assert!(matches!(err, OracleError::PublicKeyNotFound));
    }

    #[test]
    fn oracle_incr_request_id_increments() {
        let pk = PublicKey::from_base58(
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )
        .unwrap();
        let host = setup_host_with_pk(&pk);
        let mut oracle = Oracle::new(&host).unwrap();
        assert_eq!(oracle.incr_request_id(), 0);
        assert_eq!(oracle.incr_request_id(), 1);
        assert_eq!(oracle.incr_request_id(), 2);
    }
}
