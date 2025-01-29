use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use jstz_cli::{
    bridge::{self, deploy::DeployBridge},
    config::{self, Account, AccountConfig, Config, Network, NetworkConfig, NetworkName},
};
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
use jstz_proto::context::new_account::NewAddress;
use jstzd::{
    self,
    task::jstzd::{JstzdConfig, JstzdServer},
};
use octez::{r#async::client::OctezClient, unused_port};
use predicates::prelude::*;
use serde_json::json;
use std::{fs, path::Path, process::Command, str::FromStr};
use tempfile::TempDir;

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[tokio::test(flavor = "multi_thread")]
async fn fa_deploy_test() {
    std::env::remove_var("USE_JSTZD");

    // 0. Start jstzd
    let mut jstzd_server = jstzd_server().await;
    jstzd_server.run(false).await.unwrap();
    let jstzd_config = jstzd_server.get_config().await;

    // 1. Deploy Tezos FA2.1 contract
    let octez_client = OctezClient::new(jstzd_config.octez_client_config().clone());
    let init_data =
        "Pair 1000000000 { Elt \"tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx\" 1000000000 } {}";
    let fa_token_alias = "fa-token";
    let bootstrap1_alias = "bootstrap1";
    let tezos_fa_path = format!(
        "{}/tests/resources/fa2.1/tezos_fa_token.tz",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    octez_client
        .originate_contract(
            fa_token_alias,
            bootstrap1_alias,
            0.0,
            Path::new(tezos_fa_path.as_str()),
            Some(init_data),
            Some(999.0),
        )
        .await
        .unwrap();

    // 2. Deploy Jstz FA2.1 contract
    let jstz_fa_path = format!(
        "{}/tests/resources/fa2.1/jstz_fa_token.minjs",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    let jstz_config = build_config_from_jstzd(&jstzd_config);
    let tmpdir = TempDir::new().unwrap();
    let tmpdir_raw = tmpdir.path().to_str().unwrap();
    fs::write(
        tmpdir.path().join("config.json"),
        serde_json::to_string(&jstz_config).unwrap(),
    )
    .unwrap();

    std::env::set_var("JSTZ_HOME", tmpdir_raw);
    let deploy_command = jstz_cli::Command::Deploy {
        code: Some(jstz_fa_path),
        balance: 0,
        name: Some(fa_token_alias.to_string()),
        network: None,
    };
    jstz_cli::exec(deploy_command).await.unwrap();

    // 3. Deploy FA bridge
    let deploy_bridge_command = bridge::Command::FaDeploy(DeployBridge {
        source: FromStr::from_str(bootstrap1_alias).unwrap(),
        ticket_id: 1,
        ticket_content: None,
        total_ticket_supply: 1000000000,
        tezos_fa_token: FromStr::from_str(fa_token_alias).unwrap(),
        fa_token_id: Some(1),
        jstz_fa_token: FromStr::from_str(fa_token_alias).unwrap(),
        network: None,
    });
    jstz_cli::exec(jstz_cli::Command::Bridge(deploy_bridge_command))
        .await
        .unwrap();

    // 4. Execute FA token Deposit
    let jstz_address = jstzd_config.octez_rollup_config().address.to_string();
    let bridge_alias = format!("{}-bridge", fa_token_alias);
    let receiver_addr = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
    let args = format!("Pair \"{}\" \"{}\" 1000", jstz_address, receiver_addr);
    octez_client
        .call_contract(
            bootstrap1_alias,
            &bridge_alias,
            0.0,
            "deposit",
            &args,
            Some(999.0),
        )
        .await
        .unwrap();

    // 5. Verify: Check balance of receiver
    Command::cargo_bin("jstz")
        .unwrap()
        .args([
            "run",
            &format!(
                "tezos://{}/balances/{}",
                fa_token_alias,
                receiver_addr
            ),
        ])
        .envs([("JSTZ_HOME", tmpdir_raw)])
        .assert()
        // cli prints things to std err
        .stderr(predicate::str::contains(
            r#"Body: {"balance":1000,"request":{"owner":"tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx","token_id":1}}"#,
        ));

    jstzd_server.stop().await.unwrap();
    std::env::remove_var("JSTZ_HOME");
}

async fn jstzd_server() -> JstzdServer {
    let (port, jstzd_config) = jstzd::build_config(
        jstzd::parse_json_config(json!({
            "server_port": unused_port()
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    JstzdServer::new(jstzd_config, port)
}

fn build_config_from_jstzd(jstzd_config: &JstzdConfig) -> Config {
    // Set jstz config to jstzd values
    let octez_client_dir = Path::new(
        jstzd_config
            .octez_client_config()
            .base_dir()
            .to_string()
            .as_str(),
    )
    .to_owned();
    let accounts = AccountConfig {
        current_alias: Some("test".to_string()),
        accounts: [(
            "test".to_string(),
            Account::User(config::User {
                address: NewAddress::from_base58("tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9")
                    .unwrap(),
                secret_key: SecretKey::from_base58(
                    "edsk3zmEDXpWukDYviGXHRoBt2UMggJwwZsKvyAgaDDLLcQ6FMLDqS",
                )
                .unwrap(),
                public_key: PublicKey::from_base58(
                    "edpkuXDAprNEQejWbm4wrTdeQAKABByijQd3xC8hZEttgrNVA9B6gi",
                )
                .unwrap(),
            }),
        )]
        .into_iter()
        .collect(),
    };
    let networks = NetworkConfig {
        default_network: Some(NetworkName::Custom("test-int".to_string())),
        networks: [(
            "test-int".to_string(),
            Network {
                octez_node_rpc_endpoint: jstzd_config
                    .octez_node_config()
                    .rpc_endpoint
                    .to_string(),
                jstz_node_endpoint: jstzd_config.jstz_node_config().endpoint.to_string(),
            },
        )]
        .into_iter()
        .collect(),
    };

    Config::new(Some(octez_client_dir), None, accounts, networks)
}
