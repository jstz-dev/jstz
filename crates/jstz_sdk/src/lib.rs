use jstz_crypto::secret_key::SecretKey;
use jstz_crypto::verifier::passkey::parse_passkey_signature as parse_passkey_signature_inner;
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

/// Parses signature returned from the passkey device into a valid base58
/// Tezos P256 signature. The passkey signature must be using P256 (alg = -7)
#[wasm_bindgen]
pub fn parse_passkey_signature(signature: JsValue) -> Result<String, JsValue> {
    let signature: String = serde_wasm_bindgen::from_value(signature)?;
    let parsed_signature = parse_passkey_signature_inner(signature.as_str())
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(parsed_signature.to_base58_check())
}
