use jstz_crypto::secret_key::SecretKey;
use jstz_proto::operation::Operation;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn sign_operation(operation: JsValue, secret_key: &str) -> Result<String, JsValue> {
    let json: serde_json::Value = serde_wasm_bindgen::from_value(operation)?;
    let operation: Operation =
        serde_json::from_value(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let hash = operation.hash();
    let secret_key = SecretKey::from_base58(secret_key)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let signature = secret_key
        .sign(hash)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(signature.to_base58())
}

#[wasm_bindgen]
pub fn hash_operation(operation: JsValue) -> Result<String, JsValue> {
    let operation: Operation = serde_wasm_bindgen::from_value(operation)?;
    Ok(operation.hash().to_string())
}
