use bincode::{Decode, Encode};
use passkey::{verify_passkey, AuthenticatorAssertionResponseRaw};
use serde::{Deserialize, Serialize};

use utoipa::ToSchema;

use crate::{public_key::PublicKey, signature::Signature, Result};

pub mod passkey;

#[derive(
    Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema, Encode, Decode, Clone,
)]
pub enum Verifier {
    Passkey(AuthenticatorAssertionResponseRaw),
}

impl Verifier {
    pub fn verify(
        &self,
        message: &[u8],
        public_key: &PublicKey,
        signature: &Signature,
    ) -> Result<()> {
        match (&self, public_key, signature) {
            (
                Verifier::Passkey(authenticator_assertion_response_raw),
                PublicKey::P256(p256_pk),
                Signature::P256(p256_sig),
            ) => verify_passkey(
                authenticator_assertion_response_raw,
                p256_pk,
                p256_sig,
                message,
            ),
            _ => Err(crate::Error::InvalidVerfier),
        }
    }
}
