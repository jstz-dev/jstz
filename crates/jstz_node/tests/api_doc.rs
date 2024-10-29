use std::path::PathBuf;

use pretty_assertions::assert_eq;

#[test]
fn api_doc_regression() {
    let _ = include_str!("../openapi.json");
    let filename = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
    let current_spec = std::fs::read_to_string(filename).unwrap();
    let current_spec = current_spec.trim();
    let generated_spec = jstz_node::openapi_json_raw().unwrap();
    assert_eq!(
        current_spec,
        generated_spec,
        "API doc regression detected. Run the 'spec' command to update:\n\tcargo run --bin jstz-node -- spec -o crates/jstz_node/openapi.json"
    );
}
