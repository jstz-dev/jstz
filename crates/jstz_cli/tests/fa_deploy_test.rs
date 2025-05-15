// use assert_cmd::cargo::CommandCargoExt;
// use jstz_cli::{
//     bridge::deploy::DeployBridge,
//     config::{self, Account, AccountConfig, Config, Network, NetworkConfig, NetworkName},
// };
// use serde_json::Value;

// use jstz_crypto::hash::Hash;
// use jstz_crypto::{
//     public_key::PublicKey, public_key_hash::PublicKeyHash, secret_key::SecretKey,
// };
// use jstzd::{
//     self,
//     task::jstzd::{JstzdConfig, JstzdServer},
// };
// use octez::r#async::client::OctezClient;
// use std::{fs, path::Path, process::Command, str::FromStr};
// use tempfile::NamedTempFile;

// fn create_temp_config_file(config: &Config) -> NamedTempFile {
//     let temp_file = NamedTempFile::new().unwrap();
//     fs::write(temp_file.path(), serde_json::to_string(config).unwrap()).unwrap();
//     temp_file
// }

// #[cfg_attr(feature = "skip-rollup-tests", ignore)]
// #[tokio::test(flavor = "multi_thread")]
// #[cfg(test)]
// async fn fa_deploy_test() {
//     // 0. Start jstzd
//     let (port, jstzd_config) = jstzd::build_config(Default::default()).await.unwrap();
//     let mut jstzd_server = JstzdServer::new(jstzd_config.clone(), port);
//     jstzd_server.run(false).await.unwrap();

//     // 1. Deploy FA2.1 contract
//     // The `TEST` (0x54455354) token has a total supply of `1000` tokens and is minted to `bootstrap1`.
//     // https://github.com/oxheadalpha/smart-contracts/blob/master/multi_asset/ligo/src/fa2_multi_asset.mligo
//     let bootstrap1_alias = "bootstrap1";
//     let bootstrap1 = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";

//     let octez_client = OctezClient::new(jstzd_config.octez_client_config().clone());
//     let init_data = format!(
//         "(Pair (Pair \"{}\" None False) \
//         (Pair {{ Elt (Pair \"{}\" 1) 1000 }} \
//               {{}} \
//               {{ Elt 1 1000 }} \
//               {{ Elt 1 (Pair 1 {{ Elt \"decimals\" 0x31 ; Elt \"name\" 0x54455354 ; Elt \"symbol\" 0x54455354 }}) }}) \
//         {{}})",
//         bootstrap1, bootstrap1
//     );
//     let fa_token_alias = "fa-token";
//     let tezos_fa_path = format!(
//         "{}/tests/resources/fa2.1/tezos_fa_token.tz",
//         std::env::var("CARGO_MANIFEST_DIR").unwrap()
//     );
//     let (fa_address, _) = octez_client
//         .originate_contract(
//             fa_token_alias,
//             bootstrap1_alias,
//             0.0,
//             Path::new(tezos_fa_path.as_str()),
//             Some(&init_data),
//             Some(999.0),
//         )
//         .await
//         .unwrap();

//     // 2. Deploy Jstz FA2.1 contract
//     let jstz_fa_path = format!(
//         "{}/tests/resources/fa2.1/fa_jstz_token.minjs",
//         std::env::var("CARGO_MANIFEST_DIR").unwrap()
//     );
//     let jstz_config = build_config_from_jstzd(&jstzd_config);
//     let temp_file = create_temp_config_file(&jstz_config);
//     let temp_file_path = temp_file.path().to_path_buf();

//     // 3. Deploy the bridge and ticket contracts
//     let deploy_command = jstz_cli::Command::Deploy {
//         code: Some(jstz_fa_path),
//         balance: None,
//         name: Some(fa_token_alias.to_string()),
//         network: None,
//         force: false,
//         config: Some(temp_file_path.clone()),
//     };
//     jstz_cli::exec(deploy_command).await.unwrap();
//     let deploy_bridge = DeployBridge {
//         source: FromStr::from_str(bootstrap1_alias).unwrap(),
//         ticket_id: 1,
//         ticket_content: None,
//         total_ticket_supply: 1000,
//         tezos_fa_token: FromStr::from_str(fa_token_alias).unwrap(),
//         fa_token_id: Some(1),
//         jstz_fa_token: FromStr::from_str(fa_token_alias).unwrap(),
//         network: None,
//         config: Some(temp_file_path),
//     };

//     // 4. Approve the transfer of FA token from the bootstrap1 account to the bridge contract
//     let bridge_address = deploy_bridge.exec().await.unwrap();
//     octez_client
//         .call_contract(
//             bootstrap1_alias,
//             &fa_address.to_base58_check(),
//             0.0,
//             "update_operators",
//             &format!(
//                 "{{ Left (Pair \"{}\" \"{}\" 1) }}",
//                 bootstrap1, bridge_address
//             ),
//             Some(999.0),
//         )
//         .await
//         .unwrap();

//     // 4. Execute FA token Deposit
//     let jstz_address = jstzd_config.octez_rollup_config().address.to_string();
//     let bridge_alias = format!("{}-bridge", fa_token_alias);
//     let receiver_addr = bootstrap1;
//     let args = format!("Pair \"{}\" \"{}\" 1000", jstz_address, receiver_addr);
//     octez_client
//         .call_contract(
//             bootstrap1_alias,
//             &bridge_alias,
//             0.0,
//             "deposit",
//             &args,
//             Some(999.0),
//         )
//         .await
//         .unwrap();

//     // 5. Verify: Check balance of receiver
//     let expected_json: Value = serde_json::json!({
//         "balance": 1000,
//         "request": {
//             "owner": bootstrap1,
//             "token_id": 1
//         }
//     });
//     let output = Command::cargo_bin("jstz")
//         .unwrap()
//         .args([
//             "run",
//             &format!("jstz://{}/balances/{}", fa_token_alias, receiver_addr),
//         ])
//         .output()
//         .unwrap();
//     let stderr_str = String::from_utf8_lossy(&output.stderr);
//     let parsed: Value =
//         serde_json::from_str(&stderr_str).expect("Invalid JSON format in stderr");
//     assert_eq!(parsed, expected_json);
//     jstzd_server.stop().await.unwrap();
// }

// fn build_config_from_jstzd(jstzd_config: &JstzdConfig) -> Config {
//     // Set jstz config to jstzd values
//     let octez_client_dir = Path::new(
//         jstzd_config
//             .octez_client_config()
//             .base_dir()
//             .to_string()
//             .as_str(),
//     )
//     .to_owned();
//     let accounts = AccountConfig {
//         current_alias: Some("test".to_string()),
//         accounts: [(
//             "test".to_string(),
//             Account::User(config::User {
//                 address: PublicKeyHash::from_base58(
//                     "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9",
//                 )
//                 .unwrap(),
//                 secret_key: SecretKey::from_base58(
//                     "edsk3zmEDXpWukDYviGXHRoBt2UMggJwwZsKvyAgaDDLLcQ6FMLDqS",
//                 )
//                 .unwrap(),
//                 public_key: PublicKey::from_base58(
//                     "edpkuXDAprNEQejWbm4wrTdeQAKABByijQd3xC8hZEttgrNVA9B6gi",
//                 )
//                 .unwrap(),
//             }),
//         )]
//         .into_iter()
//         .collect(),
//     };
//     let networks = NetworkConfig {
//         default_network: Some(NetworkName::Custom("test-int".to_string())),
//         networks: [(
//             "test-int".to_string(),
//             Network {
//                 octez_node_rpc_endpoint: jstzd_config
//                     .octez_node_config()
//                     .rpc_endpoint
//                     .to_string(),
//                 jstz_node_endpoint: jstzd_config.jstz_node_config().endpoint.to_string(),
//             },
//         )]
//         .into_iter()
//         .collect(),
//     };

//     Config::new(Some(octez_client_dir), accounts, networks)
// }
