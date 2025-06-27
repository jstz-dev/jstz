#[cfg(feature = "v2_runtime")]
mod oracle_tests {
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::time::Duration;

    use jstz_crypto::keypair_from_mnemonic;
    use jstz_node::config::{JstzNodeConfig, KeyPair};
    use jstzd::jstz_rollup_path;
    use jstzd::task::jstzd::{JstzdConfig, JstzdServer};
    use jstzd::task::utils::retry;
    use jstzd::{BOOTSTRAP_CONTRACT_NAMES, JSTZ_ROLLUP_ADDRESS};
    use octez::r#async::baker::{BakerBinaryPath, OctezBakerConfigBuilder};
    use octez::r#async::client::OctezClientConfigBuilder;
    use octez::r#async::endpoint::Endpoint;
    use octez::r#async::file::FileWrapper;
    use octez::r#async::node_config::{
        OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder,
    };
    use octez::r#async::protocol::{
        BootstrapAccount, BootstrapContract, BootstrapSmartRollup,
        ProtocolParameterBuilder, SmartRollupPvmKind,
    };
    use octez::r#async::rollup::{OctezRollupConfigBuilder, RollupDataDir};
    use octez::unused_port;
    use serde_json::Value;
    use std::fs;
    use tempfile::{NamedTempFile, TempDir};
    use tezos_crypto_rs::hash::SmartRollupHash;
    use tokio::time::{sleep, timeout};

    const ACTIVATOR_PK: &str = "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2";
    pub const JSTZ_ROLLUP_OPERATOR_PK: &str =
        "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav";
    pub const JSTZ_ROLLUP_OPERATOR_ALIAS: &str = "bootstrap1";

    /// Test that jstzd can start with oracle key pair configuration
    #[cfg_attr(feature = "skip-rollup-tests", ignore)]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_jstzd_with_oracle_key_pair() {
        let octez_node_rpc_endpoint = Endpoint::localhost(unused_port());
        let rollup_rpc_endpoint = Endpoint::try_from(
            http::Uri::from_str(&format!("http://127.0.0.1:{}", unused_port())).unwrap(),
        )
        .unwrap();
        let jstz_node_rpc_endpoint = Endpoint::localhost(unused_port());
        let jstzd_port = unused_port();

        // Generate a test oracle key pair
        let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
        let (oracle_public_key, oracle_secret_key) =
            keypair_from_mnemonic(mnemonic, "").unwrap();

        let (mut jstzd, config) = create_jstzd_server_with_oracle(
            &octez_node_rpc_endpoint,
            &rollup_rpc_endpoint,
            &jstz_node_rpc_endpoint,
            jstzd_port,
            Some(KeyPair(oracle_public_key, oracle_secret_key)),
        )
        .await;

        // Start jstzd with oracle configuration
        jstzd.run(false).await.unwrap();

        // Ensure all components are up and running
        ensure_jstzd_components_are_up(&jstzd, &octez_node_rpc_endpoint, jstzd_port)
            .await;

        // Verify that the oracle key pair is properly configured
        let jstz_node_config = config.jstz_node_config();
        assert!(jstz_node_config.oracle_key_pair.is_some());

        let KeyPair(configured_pk, configured_sk) =
            jstz_node_config.oracle_key_pair.as_ref().unwrap();
        assert_eq!(
            configured_pk.to_string(),
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi"
        );
        assert_eq!(
            configured_sk.to_string(),
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM"
        );

        // Test that the health endpoint is accessible
        let client = reqwest::Client::new();
        let health_response = client
            .get(format!("http://localhost:{}/health", jstzd_port))
            .send()
            .await
            .unwrap();
        assert!(health_response.status().is_success());

        // Shutdown after a short delay to ensure oracle node had time to start
        tokio::spawn(async move {
            sleep(Duration::from_secs(3)).await;
            reqwest::Client::new()
                .put(format!("http://localhost:{}/shutdown", jstzd_port))
                .send()
                .await
                .unwrap();
        });

        timeout(Duration::from_secs(30), jstzd.wait())
            .await
            .expect("should not wait too long for the server to be taken down");

        ensure_jstzd_components_are_down(&jstzd, &octez_node_rpc_endpoint, jstzd_port)
            .await;
    }

    /// Test that jstzd can start without oracle key pair configuration
    #[cfg_attr(feature = "skip-rollup-tests", ignore)]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_jstzd_without_oracle_key_pair() {
        let octez_node_rpc_endpoint = Endpoint::localhost(unused_port());
        let rollup_rpc_endpoint = Endpoint::try_from(
            http::Uri::from_str(&format!("http://127.0.0.1:{}", unused_port())).unwrap(),
        )
        .unwrap();
        let jstz_node_rpc_endpoint = Endpoint::localhost(unused_port());
        let jstzd_port = unused_port();

        let (mut jstzd, config) = create_jstzd_server_with_oracle(
            &octez_node_rpc_endpoint,
            &rollup_rpc_endpoint,
            &jstz_node_rpc_endpoint,
            jstzd_port,
            None, // No oracle key pair
        )
        .await;

        // Start jstzd without oracle configuration
        jstzd.run(false).await.unwrap();

        // Ensure all components are up and running
        ensure_jstzd_components_are_up(&jstzd, &octez_node_rpc_endpoint, jstzd_port)
            .await;

        // Verify that no oracle key pair is configured
        let jstz_node_config = config.jstz_node_config();
        assert!(jstz_node_config.oracle_key_pair.is_none());

        // Test that the health endpoint is accessible
        let client = reqwest::Client::new();
        let health_response = client
            .get(format!("http://localhost:{}/health", jstzd_port))
            .send()
            .await
            .unwrap();
        assert!(health_response.status().is_success());

        // Shutdown
        tokio::spawn(async move {
            sleep(Duration::from_secs(2)).await;
            reqwest::Client::new()
                .put(format!("http://localhost:{}/shutdown", jstzd_port))
                .send()
                .await
                .unwrap();
        });

        timeout(Duration::from_secs(30), jstzd.wait())
            .await
            .expect("should not wait too long for the server to be taken down");

        ensure_jstzd_components_are_down(&jstzd, &octez_node_rpc_endpoint, jstzd_port)
            .await;
    }

    /// Test oracle configuration serialization
    #[test]
    fn test_oracle_config_serialization() {
        let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
        let (oracle_public_key, oracle_secret_key) =
            keypair_from_mnemonic(mnemonic, "").unwrap();

        let config = JstzNodeConfig::new(
            &Endpoint::localhost(8932),
            &Endpoint::localhost(8933),
            PathBuf::from("/tmp/preimages").as_path(),
            PathBuf::from("/tmp/kernel.log").as_path(),
            KeyPair::default(),
            jstz_node::RunMode::Default,
            0,
            PathBuf::from("/tmp/debug.log").as_path(),
            Some(KeyPair(oracle_public_key, oracle_secret_key)),
        );

        let json = serde_json::to_value(&config).unwrap();

        // Verify oracle key pair is serialized as an array
        let oracle_key_pair = &json["oracle_key_pair"];
        assert!(oracle_key_pair.is_array());
        let oracle_array = oracle_key_pair.as_array().unwrap();
        assert_eq!(oracle_array.len(), 2);
        assert_eq!(
            oracle_array[0],
            "edpkuEb5VsDrcVZnbWg6sAsSG3VYVUNRKATfryPCDkzi77ZVLiXE3Z"
        );
        assert_eq!(
            oracle_array[1],
            "edsk2uqim1xRamoBVn6WEHVWEtiKq2ZCXooAzpjC3tGNSVrL9aLcKM"
        );
    }

    async fn create_jstzd_server_with_oracle(
        octez_node_rpc_endpoint: &Endpoint,
        rollup_rpc_endpoint: &Endpoint,
        jstz_node_rpc_endpoint: &Endpoint,
        jstzd_port: u16,
        oracle_key_pair: Option<KeyPair>,
    ) -> (JstzdServer, JstzdConfig) {
        let run_options = OctezNodeRunOptionsBuilder::new()
            .set_synchronisation_threshold(0)
            .set_network("sandbox")
            .build();
        let octez_node_config = OctezNodeConfigBuilder::new()
            .set_network("sandbox")
            .set_rpc_endpoint(octez_node_rpc_endpoint)
            .set_run_options(&run_options)
            .build()
            .unwrap();

        let (rollup_kernel_installer, rollup_preimages_dir, rollup_parameters_ty) =
            jstz_rollup_files();

        let protocol_params = ProtocolParameterBuilder::new()
            .set_bootstrap_accounts([
                BootstrapAccount::new(ACTIVATOR_PK, 15_000_000_000).unwrap(),
                BootstrapAccount::new(JSTZ_ROLLUP_OPERATOR_PK, 60_000_000_000).unwrap(),
            ])
            .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
                JSTZ_ROLLUP_ADDRESS,
                SmartRollupPvmKind::Wasm,
                fs::read_to_string(rollup_kernel_installer.as_path())
                    .unwrap()
                    .as_str(),
                Value::from_str(
                    fs::read_to_string(rollup_parameters_ty).unwrap().as_str(),
                )
                .expect("failed to stringify JSON"),
            )
            .unwrap()])
            .set_bootstrap_contracts(read_bootstrap_contracts().await)
            .build()
            .unwrap();

        let octez_client_config =
            OctezClientConfigBuilder::new(octez_node_config.rpc_endpoint.clone())
                .build()
                .unwrap();

        let baker_config = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Env(protocol_params.protocol().clone()))
            .set_octez_client_base_dir(
                PathBuf::from(octez_client_config.base_dir())
                    .to_str()
                    .unwrap(),
            )
            .set_octez_node_endpoint(&octez_node_config.rpc_endpoint)
            .build()
            .expect("Failed to build baker config");

        let kernel_debug_file = FileWrapper::default();
        let kernel_debug_file_path = kernel_debug_file.path();
        let debug_log_path = NamedTempFile::new()
            .unwrap()
            .into_temp_path()
            .keep()
            .unwrap();
        let preimages_dir = TempDir::new().unwrap();
        let preimages_dir_path = preimages_dir.path().to_path_buf();

        let rollup_config = OctezRollupConfigBuilder::new(
            octez_node_rpc_endpoint.clone(),
            octez_client_config.base_dir().into(),
            SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap(),
            JSTZ_ROLLUP_OPERATOR_ALIAS.to_string(),
            rollup_kernel_installer,
            None,
        )
        .set_data_dir(RollupDataDir::TempWithPreImages {
            preimages_dir: rollup_preimages_dir,
        })
        .set_rpc_endpoint(rollup_rpc_endpoint)
        .set_kernel_debug_file(kernel_debug_file)
        .build()
        .expect("failed to build rollup config");

        let jstz_node_config = JstzNodeConfig::new(
            jstz_node_rpc_endpoint,
            &rollup_config.rpc_endpoint,
            &preimages_dir_path,
            &kernel_debug_file_path,
            KeyPair::default(),
            jstz_node::RunMode::Default,
            0,
            &debug_log_path,
            oracle_key_pair,
        );

        let config = JstzdConfig::new(
            octez_node_config,
            baker_config,
            octez_client_config.clone(),
            rollup_config.clone(),
            jstz_node_config,
            protocol_params,
        );

        (JstzdServer::new(config.clone(), jstzd_port), config)
    }

    async fn ensure_jstzd_components_are_up(
        jstzd: &JstzdServer,
        octez_node_rpc_endpoint: &Endpoint,
        jstzd_port: u16,
    ) {
        let jstzd_health_check_endpoint =
            format!("http://localhost:{}/health", jstzd_port);
        let octez_node_health_check_endpoint =
            format!("{}/health/ready", octez_node_rpc_endpoint);

        let jstzd_running = retry(30, 1000, || async {
            let res = reqwest::get(&jstzd_health_check_endpoint).await;
            Ok(res.is_ok())
        })
        .await;
        assert!(jstzd_running);

        // check if individual components are up / jstzd health check indeed covers all components
        assert!(reqwest::get(&octez_node_health_check_endpoint)
            .await
            .is_ok());
        assert!(jstzd.baker_healthy().await);
        let rollup_running =
            retry(10, 1000, || async { Ok(jstzd.rollup_healthy().await) }).await;
        assert!(rollup_running);
        assert!(jstzd.jstz_node_healthy().await);
        assert!(jstzd.health_check().await);
    }

    async fn ensure_jstzd_components_are_down(
        jstzd: &JstzdServer,
        octez_node_rpc_endpoint: &Endpoint,
        jstzd_port: u16,
    ) {
        let jstzd_health_check_endpoint =
            format!("http://localhost:{}/health", jstzd_port);
        let octez_node_health_check_endpoint =
            format!("{}/health/ready", octez_node_rpc_endpoint);

        let jstzd_stopped = retry(30, 1000, || async {
            let res = reqwest::get(&jstzd_health_check_endpoint).await;
            if let Err(e) = res {
                return Ok(e.to_string().contains("Connection refused"));
            }
            Ok(false)
        })
        .await;
        assert!(jstzd_stopped);

        // check if individual components are terminated
        // and jstzd indeed tears down all components before it shuts down
        let node_destroyed = retry(30, 1000, || async {
            let res = reqwest::get(&octez_node_health_check_endpoint).await;
            // Should get an error since the node should have been terminated
            if let Err(e) = res {
                return Ok(e.to_string().contains("Connection refused"));
            }
            Ok(false)
        })
        .await;
        assert!(node_destroyed);
        assert!(!jstzd.baker_healthy().await);
        assert!(!jstzd.rollup_healthy().await);
        assert!(!jstzd.jstz_node_healthy().await);
        assert!(!jstzd.health_check().await);
    }

    fn jstz_rollup_files() -> (PathBuf, PathBuf, PathBuf) {
        (
            jstz_rollup_path::kernel_installer_path(),
            jstz_rollup_path::preimages_path(),
            jstz_rollup_path::parameters_ty_path(),
        )
    }

    async fn read_bootstrap_contracts() -> Vec<BootstrapContract> {
        let mut contracts = Vec::new();
        for (contract_name, _hash) in BOOTSTRAP_CONTRACT_NAMES {
            let contract_path = format!("../contracts/{}", contract_name);
            let contract_content = fs::read_to_string(contract_path).unwrap();
            let script = serde_json::from_str(&contract_content).unwrap();
            contracts.push(BootstrapContract::new(script, 1_000_000, None).unwrap());
        }
        contracts
    }
}
