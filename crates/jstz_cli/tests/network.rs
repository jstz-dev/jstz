use std::{
    fs::{create_dir_all, File},
    io::Write,
};
use tempfile::TempDir;
#[path = "./utils.rs"]
mod utils;

#[test]
fn list_networks() {
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    create_dir_all(path.parent().expect("should find parent dir"))
        .expect("should create dir");
    let mut file = File::create(&path).expect("should create file");
    file.write_all(
        serde_json::json!({
            "networks": {
                "foo": {
                    "octez_node_rpc_endpoint": "http://octez.foo.test",
                    "jstz_node_endpoint": "http://jstz.foo.test"
                },
                "very_long_name_very_long_name_very_long_name": {
                    "octez_node_rpc_endpoint": "http://octez.long.long.long.long.test",
                    "jstz_node_endpoint": "http://jstz.long.long.long.long.test"
                }
            }
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    file.flush().unwrap();

    let mut process = utils::jstz_cmd(["network", "list"], Some(tmp_dir));
    let output = process.exp_eof().unwrap().replace("\r\n", "\n");
    assert_eq!(
        output,
        r#"  +----------------------+---------------------------+---------------------------+
  | Name                 | Octez RPC endpoint        | Jstz node endpoint        |
  +======================+===========================+===========================+
  | foo                  | http://octez.foo.test     | http://jstz.foo.test      |
  +----------------------+---------------------------+---------------------------+
  | very_long_name_ve... | http://octez.long.long... | http://jstz.long.long.... |
  +----------------------+---------------------------+---------------------------+

"#
    );
}
