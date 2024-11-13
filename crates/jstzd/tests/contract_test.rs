use std::path::Path;
use tempfile::TempDir;
mod utils;
use jstzd::{EXCHANGER_ADDRESS, JSTZ_ROLLUP_ADDRESS};
use octez::r#async::client::OctezClient;
use tokio::io::AsyncWriteExt;

#[tokio::test(flavor = "multi_thread")]
async fn built_contracts() {
    let (_node, client, _baker) = utils::setup().await;
    let tmp_dir = TempDir::new().unwrap();
    generate_bootstrap_contract_files(&client, tmp_dir.path()).await;
    let contract_names = ["exchanger", "jstz_native_bridge"];
    for contract_name in contract_names {
        let current = utils::read_json_file(
            Path::new(std::env!("CARGO_MANIFEST_DIR"))
                .join(format!("resources/bootstrap_contract/{contract_name}.json")),
        )
        .await;
        let expected =
            utils::read_json_file(tmp_dir.path().join(format!("{contract_name}.json")))
                .await;

        assert_eq!(
            expected, current,
            "Bootstrap contract '{contract_name}.json' is outdated. Replace the 'code' section of the json file with the latest contract code and update init storage when necessary."
        );
    }
}

// this requires a running octez node with any activated protocol version
pub async fn generate_bootstrap_contract_files(
    octez_client: &OctezClient,
    output_dir: &Path,
) {
    let contract_init_data_mapping = [
        ("exchanger", serde_json::json!({"prim":"Unit"})),
        (
            "jstz_native_bridge",
            serde_json::json!({"prim":"Pair","args":[{"string":EXCHANGER_ADDRESS},{"string":JSTZ_ROLLUP_ADDRESS},{"prim":"None"}]}),
        ),
    ];
    for (contract_name, storage) in contract_init_data_mapping {
        let path = Path::new(std::env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../contracts/{contract_name}.tz"));
        let cmd_output = octez_client
            .spawn_and_wait_command([
                "convert",
                "script",
                path.to_str().unwrap(),
                "from",
                "michelson",
                "to",
                "json",
            ])
            .await
            .unwrap()
            .into_bytes();
        let contract_code: serde_json::Value =
            serde_json::from_str(&String::from_utf8(cmd_output).unwrap()).unwrap();
        let mut script: serde_json::Value = serde_json::json!({});
        script
            .as_object_mut()
            .unwrap()
            .insert("code".to_owned(), contract_code);
        script
            .as_object_mut()
            .unwrap()
            .insert("storage".to_owned(), storage);

        let mut output_file =
            tokio::fs::File::create(output_dir.join(format!("{contract_name}.json")))
                .await
                .unwrap();
        output_file
            .write_all(serde_json::to_string(&script).unwrap().as_bytes())
            .await
            .unwrap();
        output_file.flush().await.unwrap();
    }
}
