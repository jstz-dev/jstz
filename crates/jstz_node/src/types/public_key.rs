use jstz_crypto::public_key::PublicKey as PublicKeyInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1};
use utoipa::ToSchema;

/// Tezos public key
#[api_map_to(PublicKeyInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[serde(untagged)]
pub enum PublicKey {
    #[schema(
        title = "Ed25519",
        value_type = String,
        example = json!("edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi")
    )]
    Ed25519(PublicKeyEd25519),
    #[schema(
        title = "Secp256k1",
        value_type = String,
        example = json!("sppk7aMwoVDiMGXkzwqPMrqHNE6QrZ1vAJ2CvTEeGZRLSSTM8jogmKY")
    )]
    Secp256k1(PublicKeySecp256k1),
    #[schema(
        title = "P256",
        value_type = String,
        example = json!("p2pk67ArUx3aDGyFgRco8N3pTnnnbodpP2FMZLAewV6ZAVvCxKjW3Q1")
    )]
    P256(PublicKeyP256),
}
