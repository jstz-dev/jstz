#![cfg(feature = "v2_runtime")]

use std::path::PathBuf;

use jstz_utils::KeyPair;
use octez::r#async::endpoint::Endpoint;
use serde::Serialize;
mod data_provider;
pub mod node;
pub mod relay;

#[derive(Clone, Serialize)]
pub struct OracleNodeConfig {
    /// The Oracle signer used to authenticate valid oracle responses
    pub key_pair: Option<KeyPair>,
    pub log_path: PathBuf,
    pub jstz_node_endpoint: Endpoint,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jstz_crypto::keypair_from_mnemonic;
    use jstz_utils::KeyPair;
    use octez::r#async::endpoint::Endpoint;

    #[test]
    fn oracle_config_serialization_test() {
        let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
        let (oracle_pk, oracle_sk) = keypair_from_mnemonic(mnemonic, "").unwrap();

        let cfg = super::OracleNodeConfig {
            key_pair: Some(KeyPair(oracle_pk, oracle_sk)),
            log_path: PathBuf::from("/tmp/debug.log"),
            jstz_node_endpoint: Endpoint::localhost(1234),
        };

        let json = serde_json::to_value(&cfg).unwrap();
        let oracle_pk = json["key_pair"].as_str().expect("oracle should be string");
        assert_eq!(
            oracle_pk,
            "edpkuEb5VsDrcVZnbWg6sAsSG3VYVUNRKATfryPCDkzi77ZVLiXE3Z"
        );
    }
}
