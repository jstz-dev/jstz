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
            _ => Err(crate::Error::InvalidVerifier),
        }
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use crate::public_key::PublicKey;

    use super::{passkey::parse_passkey_signature, Verifier};

    #[test]
    fn passkey_verification() {
        let signature = crate::signature::Signature::P256(parse_passkey_signature("MEUCIQDv38zGXtPOEc3vO0SVloXyH2ipxd2ACyyDr1HlwrRCHgIgeYcrdOvoPm8nY_jhjtKbqJwVNrGYaf6Yv0l0EKAmNNk").unwrap());
        let public_key = PublicKey::from_base58(
            "p2pk66MGWLuippApsduRsaN58P1dkVrAqDxBeSKFR4164Kx22uBmzTV",
        )
        .unwrap();
        let operation_hash =
            "84540ed046dc05bf6e2dc356c4eaa3f9b44df68c4c16b7f3ca8f6c7ef85591e9";
        let raw_operation_hash = hex::decode(operation_hash).unwrap();
        let verifier = Verifier::Passkey(serde_json::from_value(json!({
                "authenticatorData": "SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAAA",
                "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiT0RRMU5EQmxaREEwTm1Sak1EVmlaalpsTW1Sak16VTJZelJsWVdFelpqbGlORFJrWmpZNFl6UmpNVFppTjJZelkyRTRaalpqTjJWbU9EVTFPVEZsT1EiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQzMjEiLCJjcm9zc09yaWdpbiI6ZmFsc2UsIm90aGVyX2tleXNfY2FuX2JlX2FkZGVkX2hlcmUiOiJkbyBub3QgY29tcGFyZSBjbGllbnREYXRhSlNPTiBhZ2FpbnN0IGEgdGVtcGxhdGUuIFNlZSBodHRwczovL2dvby5nbC95YWJQZXgifQ"
        })).unwrap());

        verifier
            .verify(raw_operation_hash.as_slice(), &public_key, &signature)
            .expect("Verifications should pass");
    }

    #[test]
    fn passkey_verification_fails_on_unmatched_sig_kind() {
        let signature = crate::signature::Signature::P256(parse_passkey_signature("MEUCIQDv38zGXtPOEc3vO0SVloXyH2ipxd2ACyyDr1HlwrRCHgIgeYcrdOvoPm8nY_jhjtKbqJwVNrGYaf6Yv0l0EKAmNNk").unwrap());
        let public_key = PublicKey::from_base58(
            "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
        )
        .unwrap();
        let operation_hash =
            "84540ed046dc05bf6e2dc356c4eaa3f9b44df68c4c16b7f3ca8f6c7ef85591e9";
        let raw_operation_hash = hex::decode(operation_hash).unwrap();
        let verifier = Verifier::Passkey(serde_json::from_value(json!({
                "authenticatorData": "SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAAA",
                "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiT0RRMU5EQmxaREEwTm1Sak1EVmlaalpsTW1Sak16VTJZelJsWVdFelpqbGlORFJrWmpZNFl6UmpNVFppTjJZelkyRTRaalpqTjJWbU9EVTFPVEZsT1EiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQzMjEiLCJjcm9zc09yaWdpbiI6ZmFsc2UsIm90aGVyX2tleXNfY2FuX2JlX2FkZGVkX2hlcmUiOiJkbyBub3QgY29tcGFyZSBjbGllbnREYXRhSlNPTiBhZ2FpbnN0IGEgdGVtcGxhdGUuIFNlZSBodHRwczovL2dvby5nbC95YWJQZXgifQ"
        })).unwrap());

        let err = verifier
            .verify(raw_operation_hash.as_slice(), &public_key, &signature)
            .expect_err("Verifier should fail to match");

        assert!(matches!(err, crate::Error::InvalidVerifier))
    }
}
