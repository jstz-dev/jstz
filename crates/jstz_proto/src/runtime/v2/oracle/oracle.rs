#![allow(unused)]
use futures::channel::oneshot::Sender;
use jstz_core::{host::HostRuntime, kv::Storage};
use jstz_crypto::public_key::PublicKey;
use std::{collections::BTreeMap, fmt::Display};
use tezos_smart_rollup::storage::path::{concat, OwnedPath};

use super::{OracleRequest, RequestId};
use crate::{
    runtime::v2::fetch::http::Response,
    storage::{ORACLE_PUBLIC_KEY_PATH, ORACLE_REQUESTS_PATH},
};

#[derive(Debug)]
pub struct Oracle {
    /// Oracle's public key
    public_key: PublicKey,
    /// Sender channels for in-flight requests
    senders: BTreeMap<RequestId, Sender<Response>>,
    next_request_id: RequestId,
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
        })
    }

    // Increments and returns the previous [`next_request_id`]
    fn incr_request_id(&mut self) -> RequestId {
        let curr = self.next_request_id;
        self.next_request_id += 1;
        curr
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
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum OracleError {
    #[error("Oracle signer public key not found at '{ORACLE_PUBLIC_KEY_PATH}'")]
    PublicKeyNotFound,
    #[error("{0}")]
    V1Error(String),
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
        assert_eq!(OracleError::PublicKeyNotFound, err);
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
