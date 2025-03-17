use anyhow::Context;
use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
use jstzd::task::{utils::poll, Task};
use octez::r#async::{
    client::{OctezClient, OctezClientConfigBuilder, Signature},
    endpoint::Endpoint,
    protocol::{
        BootstrapContract, BootstrapSmartRollup, ProtocolParameter,
        ProtocolParameterBuilder, SmartRollupPvmKind,
    },
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs::{read_to_string, remove_file},
    io::Write,
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};
use tezos_crypto_rs::hash::SmartRollupHash;
use tokio::io::AsyncWriteExt;
mod utils;
use std::path::PathBuf;
use utils::{
    activate_alpha, create_client, get_head_block_hash, get_operation_kind, get_request,
    import_activator, setup, spawn_octez_node, spawn_rollup, ACTIVATOR_SECRET_KEY,
};

fn read_file(path: &Path) -> Value {
    serde_json::from_str(&read_to_string(path).expect("Unable to read file"))
        .expect("Unable to parse JSON")
}

fn first_item(json: Value) -> Value {
    json.as_array().unwrap()[0].clone()
}

#[tokio::test]
async fn config_init() {
    let temp_dir = TempDir::new().unwrap();
    let expected_base_dir = temp_dir.path().to_path_buf();
    let expected_endpoint = Endpoint::localhost(3000);
    let config_file = NamedTempFile::new().unwrap();
    let _ = remove_file(config_file.path());
    let octez_client_config = OctezClientConfigBuilder::new(expected_endpoint.clone())
        .set_base_dir(expected_base_dir.clone())
        .build()
        .unwrap();
    let octez_client = OctezClient::new(octez_client_config);
    let res = octez_client.config_init(config_file.path()).await;
    assert!(res.is_ok());
    let actual: Value =
        serde_json::from_str(&read_to_string(config_file).expect("Unable to read file"))
            .expect("Unable to parse JSON");
    assert_eq!(
        actual["base_dir"],
        expected_base_dir.to_str().unwrap().to_owned()
    );
    assert_eq!(actual["endpoint"], expected_endpoint.to_string());
}

#[tokio::test]
async fn generates_keys() {
    let octez_client = create_client(&Endpoint::default());
    let base_dir = PathBuf::from(octez_client.base_dir());
    let alias = "test_alias".to_string();
    let res = octez_client.gen_keys(&alias, None).await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert_eq!(secret_keys["name"], alias);
}

#[tokio::test]
async fn show_address() {
    let octez_client = create_client(&Endpoint::default());
    let alias = "test_alias".to_string();
    let _ = octez_client.gen_keys(&alias, None).await;
    let res = octez_client.show_address(&alias, false).await;

    assert!(res.is_ok_and(|addr| {
        addr.hash.to_string().starts_with("tz1")
            && addr.public_key.to_string().starts_with("edpk")
    }));
}

#[tokio::test]
async fn show_address_with_secret_key() {
    let octez_client = create_client(&Endpoint::default());
    let alias = "test_alias".to_string();
    let _ = octez_client.gen_keys(&alias, None).await;
    let res = octez_client.show_address(&alias, true).await;
    assert!(res.is_ok_and(|addr| addr
        .secret_key
        .is_some_and(|sk| sk.to_string().starts_with("edsk"))));
}

#[tokio::test]
async fn show_address_fails_for_non_existing_alias() {
    let octez_client = create_client(&Endpoint::default());
    let res = octez_client.show_address("test_alias", true).await;
    assert!(res.is_err_and(|e| e
        .to_string()
        .contains("no public key hash alias named test_alias")))
}

#[tokio::test]
async fn generates_keys_with_custom_signature() {
    let octez_client = create_client(&Endpoint::default());
    let base_dir = PathBuf::from(octez_client.base_dir());
    let alias = "test_alias".to_string();
    let res = octez_client.gen_keys(&alias, Some(Signature::BLS)).await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert!(pub_keys["value"]
        .as_str()
        .unwrap()
        .starts_with("unencrypted:BL"));
    assert_eq!(secret_keys["name"], alias);
    assert!(secret_keys["value"]
        .as_str()
        .unwrap()
        .starts_with("unencrypted:BL"));
}

#[tokio::test]
async fn generates_keys_throws() {
    let octez_client = create_client(&Endpoint::default());
    let alias = "test_alias".to_string();
    let _ = octez_client.gen_keys(&alias, None).await;
    let res = octez_client.gen_keys(&alias, None).await;
    assert!(res.is_err_and(|e| { e.to_string().contains("\"gen\" \"keys\"") }));
}

#[tokio::test]
async fn imports_secret_key() {
    let octez_client = create_client(&Endpoint::default());
    let base_dir = PathBuf::from(octez_client.base_dir());
    let alias = "test_alias".to_string();
    let res = octez_client
        .import_secret_key(&alias, ACTIVATOR_SECRET_KEY)
        .await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert_eq!(secret_keys["name"], alias);
}

#[tokio::test]
async fn imports_secret_key_throws() {
    let octez_client = create_client(&Endpoint::default());
    let alias = "test_alias".to_string();
    let _ = octez_client
        .import_secret_key(&alias, ACTIVATOR_SECRET_KEY)
        .await;
    let res = octez_client
        .import_secret_key(&alias, ACTIVATOR_SECRET_KEY)
        .await;
    assert!(
        res.is_err_and(|e| { e.to_string().contains("\"import\" \"secret\" \"key\"") })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_balance() {
    // 1. start octez node
    let mut octez_node = spawn_octez_node().await;
    // 2. setup octez client
    let octez_client = create_client(octez_node.rpc_endpoint());
    // 3. import secret key for bootstrap1
    let bootstrap1 = "bootstrap1".to_string();
    octez_client
        .import_secret_key(
            &bootstrap1,
            "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )
        .await
        .expect("Failed to generate activator key");
    // 4. activate the alpha protocol
    import_activator(&octez_client).await;
    activate_alpha(&octez_client, None).await;

    // 5. check balance for bootstrap1
    let balance = octez_client.get_balance(&bootstrap1).await;
    assert!(balance.is_ok_and(|balance| balance == 3800000f64));
    let non_existing_alias = "non_existing_alias".to_string();
    let balance = octez_client.get_balance(&non_existing_alias).await;
    assert!(balance.is_err_and(|e| {
        e.to_string()
            .contains("no contract or key named non_existing_alias")
    }));
    let _ = octez_node.kill().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn activate_protocol() {
    // 1. start octez node
    let mut octez_node = spawn_octez_node().await;
    // 2. setup octez client
    let octez_client = create_client(octez_node.rpc_endpoint());
    // 3. import activator key
    import_activator(&octez_client).await;

    let blocks_head_endpoint =
        format!("{}/chains/main/blocks/head", octez_node.rpc_endpoint());
    let response = get_request(&blocks_head_endpoint).await;
    assert!(response.contains(
        "\"protocol\":\"PrihK96nBAFSxVL1GLJTVhu9YnzkMFiBeuJRPA8NwuZVZCE1L6i\""
    ));
    assert!(response.contains("\"level\":0"));
    // 4. activate the alpha protocol
    activate_alpha(&octez_client, None).await;

    // 5. check if the protocol is activated and the block is baked.
    // The block level progress indicates that the protocol has been activated.
    let response = get_request(&blocks_head_endpoint).await;
    assert!(response.contains(
        "\"protocol\":\"ProtoGenesisGenesisGenesisGenesisGenesisGenesk612im\""
    ));
    assert!(response.contains("\"level\":1"));
    let _ = octez_node.kill().await;
}

#[tokio::test]
async fn add_address() {
    let address =
        PublicKeyHash::from_base58("tz1cMWTNwecApUicCrHfTRwHEhBcZGjkUwCw").unwrap();
    let octez_client = create_client(&Endpoint::default());
    let alias = "test_alias".to_string();
    let res = octez_client.add_address(&alias, &address, false).await;
    assert!(res.is_ok());
    let res = octez_client.add_address(&alias, &address, false).await;
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("test_alias already exists"));
    let res = octez_client.add_address(&alias, &address, true).await;
    assert!(res.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn call_contract() {
    let (mut octez_node, octez_client, mut baker) = setup(None).await;
    let bootstrap1: String = "bootstrap1".to_string();
    let contract = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ".to_string();
    let before = octez_client.get_balance(&contract).await.unwrap();
    let amount = 1000f64;
    let op_hash = octez_client
        .call_contract(
            &bootstrap1,
            &contract,
            amount,
            "myEntryPoint",
            "1",
            Some(999f64),
        )
        .await;
    assert!(op_hash.is_ok());
    let after = octez_client.get_balance(&contract).await.unwrap();
    assert_eq!(before + amount, after);
    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn originate_contract_and_wait_for() {
    let (mut octez_node, octez_client, mut baker) = setup(None).await;
    let head = get_head_block_hash(&octez_node.rpc_endpoint().to_string()).await;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all("parameter (unit %entrypoint_1); storage int; code { CDR; NIL operation; PAIR; };".as_bytes()).unwrap();
    let (_, op) = octez_client
        .originate_contract(
            "foo",
            "bootstrap1",
            10000.0,
            config_file.path(),
            Some("1"),
            Some(0.5),
        )
        .await
        .unwrap();

    // test --check-previous
    tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        octez_client.wait_for(&op, None, Some(20)),
    )
    .await
    .expect("wait_for should complete soon enough")
    .expect("wait_for should be able to find the operation");

    // test --branch
    tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        octez_client.wait_for(&op, Some(&head), None),
    )
    .await
    .expect("wait_for should complete soon enough")
    .expect("wait_for should be able to find the operation");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // calling wait_for with the current head (after the operation was performed) should timeout
    let head = get_head_block_hash(&octez_node.rpc_endpoint().to_string()).await;
    tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        octez_client.wait_for(&op, Some(&head), None),
    )
    .await
    .expect_err("wait_for should timeout");

    // calling wait_for with the immediate previous block (after the operation was performed)
    // should time out
    tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        octez_client.wait_for(&op, None, Some(1)),
    )
    .await
    .expect_err("wait_for should timeout");

    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_rollup_inbox_message() {
    let (mut octez_node, octez_client, mut baker) = setup(None).await;

    let (block, op) = octez_client
        .send_rollup_inbox_message("bootstrap1", "0000", Some(0.1))
        .await
        .unwrap();

    tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        octez_client.wait_for(&op, Some(&block), None),
    )
    .await
    .expect("wait_for should complete soon enough")
    .expect("wait_for should be able to find the operation");

    let operation_kind =
        get_operation_kind(&octez_node.rpc_endpoint().to_string(), &block, &op)
            .await
            .unwrap();
    assert_eq!(operation_kind, "smart_rollup_add_messages");

    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}

#[derive(Deserialize)]
struct OutputProof {
    pub commitment: String,
    pub proof: String,
}

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[tokio::test(flavor = "multi_thread")]
async fn execute_rollup_outbox_message() {
    let rollup_address = "sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao";
    // this is the destination contract where the outbox messages target
    // this address is sealed in the rollup code
    let contract_address = "KT1TFAweS9bMBetdDB3ndFicJWAEMb8MtSrK";
    let installer_path = Path::new(std::env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/resources/rollup/{rollup_address}/installer.hex"
    ));
    let preimages_dir = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/resources/rollup/{rollup_address}/preimages"));
    let contract_path = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/resources/contract/{contract_address}.json"));
    let params = set_up_parameters_for_outbox_message(
        rollup_address,
        contract_address,
        &installer_path,
        &contract_path,
    )
    .await;

    let (mut octez_node, octez_client, mut baker) =
        setup(Some(params.parameter_file().path().to_path_buf())).await;
    let mut rollup = spawn_rollup(
        &octez_node,
        &octez_client,
        installer_path,
        preimages_dir,
        Some(rollup_address),
    )
    .await;

    octez_client
        .send_rollup_inbox_message("bootstrap1", "0000", Some(0.1))
        .await
        .unwrap();

    // wait until outbox message is cemented
    let proof = wait_for_outbox_proof(&rollup.rpc_endpoint().to_string())
        .await
        .unwrap();

    let (block, op) = octez_client
        .execute_rollup_outbox_message(
            &SmartRollupHash::from_base58_check(rollup_address).unwrap(),
            "bootstrap1",
            &proof.commitment,
            &format!("0x{}", &proof.proof),
            Some(0.1),
        )
        .await
        .unwrap();

    tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        octez_client.wait_for(&op, Some(&block), None),
    )
    .await
    .expect("wait_for should complete soon enough")
    .expect("wait_for should be able to find the operation");

    let operation_kind =
        get_operation_kind(&octez_node.rpc_endpoint().to_string(), &block, &op)
            .await
            .unwrap();
    assert_eq!(operation_kind, "smart_rollup_execute_outbox_message");

    let _ = rollup.kill().await;
    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}

async fn set_up_parameters_for_outbox_message(
    rollup_address: &str,
    contract_address: &str,
    installer_path: &PathBuf,
    contract_path: &PathBuf,
) -> ProtocolParameter {
    let kernel = String::from_utf8(
        tokio::fs::read(&installer_path)
            .await
            .unwrap_or_else(|e| panic!("failed to read installer file: {:?}", e)),
    )
    .unwrap();
    let contract_json = serde_json::from_slice(
        &tokio::fs::read(&contract_path)
            .await
            .unwrap_or_else(|e| panic!("failed to read contract file: {:?}", e)),
    )
    .unwrap();
    let params = ProtocolParameterBuilder::new()
        .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
            rollup_address,
            SmartRollupPvmKind::Wasm,
            &kernel,
            serde_json::json!({"prim": "bytes"}),
        )
        .unwrap()])
        .set_bootstrap_contracts([BootstrapContract::new(
            contract_json,
            1_000_000,
            Some(contract_address),
        )
        .unwrap()])
        .set_source_path(
            Path::new(std::env!("CARGO_MANIFEST_DIR"))
                .join("tests/sandbox-params.json")
                .to_str()
                .unwrap(),
        )
        .build()
        .unwrap();

    let mut content = tokio::fs::read_to_string(params.parameter_file().path())
        .await
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&content).unwrap();

    // overwriting these config values so that outbox messages get cemented sooner
    value.as_object_mut().unwrap().insert(
        "smart_rollup_challenge_window_in_blocks".to_owned(),
        serde_json::json!(8),
    );
    value.as_object_mut().unwrap().insert(
        "smart_rollup_commitment_period_in_blocks".to_owned(),
        serde_json::json!(8),
    );

    content = serde_json::to_string(&value).unwrap();
    tokio::fs::File::create(params.parameter_file().path())
        .await
        .unwrap()
        .write_all(content.as_bytes())
        .await
        .unwrap();

    params
}

async fn wait_for_outbox_proof(rollup_rpc_endpoint: &str) -> anyhow::Result<OutputProof> {
    #[derive(Deserialize)]
    struct Message {
        message_index: u32,
    }
    #[derive(Deserialize)]
    struct Executable {
        outbox_level: u32,
        messages: Vec<Message>,
    }

    let url = format!("{rollup_rpc_endpoint}/local/outbox/pending/executable");
    let (outbox_level, message_index) = poll(30, 1000, || async {
        // response: [{"outbox_level": 1, "messages": [{"message_index": 0, ...}, {"message_index": 1, ...}]}]
        let res = reqwest::get(&url).await.ok()?;
        let vs = res.json::<Vec<Executable>>().await.unwrap();
        // using the first message here since any of those should work
        let v = vs.first()?;
        let m = v.messages.first()?;
        Some((v.outbox_level, m.message_index))
    })
    .await
    .expect("should be able to find outbox message soon enough");

    let url = format!("{rollup_rpc_endpoint}/global/block/head/helpers/proofs/outbox/{outbox_level}/messages?index={message_index}");
    let res = reqwest::get(&url)
        .await
        .context("failed to call rollup RPC endpoint")?;
    let v = res
        .json::<OutputProof>()
        .await
        .context("failed to parse response of rollup outbox proof RPC")?;
    Ok(v)
}
