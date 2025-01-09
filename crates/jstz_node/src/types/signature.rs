use jstz_crypto::signature::Signature as SignatureInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(SignatureInternal)]
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(untagged)]
pub enum Signature {
    #[schema(
        title = "Ed25519 signature", 
        value_type = String,
        example = json!("edsigtpe2oRBMFdrrwf99ETNjmBaRzNDexDjhancfQdz5phrwyPPhRi9L7kzJD4cAW1fFcsyTJcTDPP8W4H168QPQdGPKe7jrZB")
    )]
    Ed25519(tezos_crypto_rs::hash::Ed25519Signature),
    #[schema(
        title = "Secp256k1 signature", 
        value_type = String,
        example = json!("spsig1NajZUT4nSiWU7UiV98fmmsjApFFYwPHtiDiJfGMgGL6oP3U9SPEccTfhAPdnAcvJ6AUSQ8EBPxYNX4UeNNDLBxVg9qv5H")
    )]
    Secp256k1(tezos_crypto_rs::hash::Secp256k1Signature),
    #[schema(
        title = "P256 signature", 
        value_type = String,
        example = json!("p2signEdtYeHXyWfCaGej9AFv7QraDsunRimyK47YGBQRNDEPXPQctwjPxbyFbTUtVLsACzG8QTrLAxddjjTRikF3nThwKL8nH")
    )]
    P256(tezos_crypto_rs::hash::P256Signature),
}
