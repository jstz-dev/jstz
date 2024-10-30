use jstz_crypto::public_key_hash::PublicKeyHash;
use jstzd::task::Task;
use octez::r#async::client::OctezClientBuilder;
use octez::r#async::client::Signature;
use octez::r#async::endpoint::Endpoint;
use serde_json::Value;
use std::{
    fs::{read_to_string, remove_file},
    io::Write,
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};
mod utils;
use std::path::PathBuf;
use utils::{
    activate_alpha, create_client, get_request, import_activator, setup,
    spawn_octez_node, SECRET_KEY,
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
    let octez_client = OctezClientBuilder::new(expected_endpoint.clone())
        .set_base_dir(expected_base_dir.clone())
        .build()
        .unwrap();
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
    let base_dir = PathBuf::try_from(octez_client.base_dir()).unwrap();
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
    let base_dir = PathBuf::try_from(octez_client.base_dir()).unwrap();
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
    let base_dir = PathBuf::try_from(octez_client.base_dir()).unwrap();
    let alias = "test_alias".to_string();
    let res = octez_client.import_secret_key(&alias, SECRET_KEY).await;
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
    let _ = octez_client.import_secret_key(&alias, SECRET_KEY).await;
    let res = octez_client.import_secret_key(&alias, SECRET_KEY).await;
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
    activate_alpha(&octez_client).await;

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
    activate_alpha(&octez_client).await;

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
async fn originate_contract() {
    let (mut octez_node, octez_client, mut baker) = setup().await;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all("parameter (unit %entrypoint_1); storage int; code { CDR; NIL operation; PAIR; };".as_bytes()).unwrap();
    octez_client
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

    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}
