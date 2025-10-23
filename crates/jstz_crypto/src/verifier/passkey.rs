//! This module implements the verifier for verifying signatures
//! signed by a passkey device that adheres to [Web Authentication API spec](
//! https://w3c.github.io/webauthn/#iface-authenticatorattestationresponse).
use bincode::{Decode, Encode};
use cryptoxide::hashing::sha2::Sha256;
use p256::ecdsa::Signature;
use serde::{Deserialize, Serialize};
use serde_with::base64::{Base64, UrlSafe};
use serde_with::formats::Unpadded;
use serde_with::serde_as;
use tezos_crypto_rs::hash::P256Signature;
use thiserror::Error;

use crate::error::Result;
use crate::public_key;
use crate::signature;

use base64::Engine;
use utoipa::ToSchema;
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
struct ClientData {
    challenge: String,
    r#type: String,
    origin: String,
    #[serde(rename = "crossOrigin")]
    cross_origin: bool,
}

#[derive(Debug, Error)]
pub enum PasskeyError {
    #[error("ClientDataError: {0}")]
    ClientDataError(serde_json::Error),

    #[error("Base64DecodeError: {0}")]
    Base64DecodeError(base64::DecodeError),

    #[error("FromHexError: {0}")]
    FromHexError(hex::FromHexError),

    #[error("Challenge does not match operation hash")]
    ChallengeMismatch,

    #[error("Bad DER signature")]
    BadDerSignature,

    #[error("SigConversionError: {0}")]
    SignatureConversionError(p256::ecdsa::Error),

    #[error("PkConversionError: {0}")]
    PublicKeyConversionError(p256::ecdsa::Error),

    #[error("Verification failed")]
    VerificationFailed,
}

use PasskeyError::*;

/// A narrowed view of the raw AuthenticatorAssertionResponse returned by
/// the passkey device. Only the fields necessary for verification are kept.
#[serde_as]
#[derive(
    Debug, PartialEq, Eq, Clone, Serialize, Deserialize, ToSchema, Encode, Decode,
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatorAssertionResponseRaw {
    #[serde_as(as = "Base64<UrlSafe, Unpadded>")]
    authenticator_data: Vec<u8>,
    /// ClientDataJSON contains metadata about the client and the cryptographic
    /// challenge in JSON encoding. For the purposes of Jstz, the challenge
    /// is the operation hash.
    #[serde_as(as = "Base64<UrlSafe, Unpadded>")]
    #[serde(rename = "clientDataJSON")]
    client_data_json: Vec<u8>,
}

impl AuthenticatorAssertionResponseRaw {
    /// Challenge that was included in the client data encopded in base64url
    pub fn challenge_base64url(&self) -> Result<String> {
        let client_data: ClientData =
            serde_json::from_slice(&self.client_data_json).map_err(ClientDataError)?;
        Ok(client_data.challenge)
    }

    /// Returns the raw message that gets signed by the passkey protocol as per the
    /// [Web Authentication API](https://developer.mozilla.org/en-US/docs/Web/API/AuthenticatorAssertionResponse/signature)
    /// Scheme: authenticator_data + sha256(client_data_json)
    pub fn message(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.authenticator_data.len() + 32);

        // Authenticator data
        data.extend_from_slice(&self.authenticator_data);

        // Hash client data json
        let mut hasher = Sha256::new();
        hasher.update_mut(&self.client_data_json);
        let client_data_json_hash = hasher.finalize();

        data.extend_from_slice(&client_data_json_hash);
        data
    }
}

/// Verifies signature returned from the passkey signing flow. The signature is applied
/// over the message
///
///     authenticator_data | sha256(client_data_json)
///
/// where `authenticator_data` and `client_data_json` are the raw unencoded bytes of their
/// respective values present as fields on [AuthenticatorAssertionResponseRaw].
///
/// To verify,
///     1. Parse the challenge from authn_assertion_resp.client_data_json. The challenge
///        must be decoded from `base64url(hex(data))`;
///             `base64url` because WebAuthn encodes ArrayBuffers this way
///             `hex` because this is the stringified form our operation hashes
///     2. Assert that challenge == operation_hash
///     3. Construct the message as above (call authn_assertion_resp.message())
///     4. Verify signature over the message
pub fn verify_passkey(
    authn_assertion_resp: &AuthenticatorAssertionResponseRaw,
    public_key: &public_key::P256,
    signature: &signature::P256,
    operation_hash: &[u8],
) -> Result<()> {
    use p256::ecdsa::{signature::Verifier, VerifyingKey};

    let challenge = authn_assertion_resp.challenge_base64url()?;
    let raw_challenge = hex::decode(
        base64::prelude::BASE64_URL_SAFE_NO_PAD
            .decode(challenge)
            .map_err(Base64DecodeError)?,
    )
    .map_err(FromHexError)?;
    if operation_hash != raw_challenge {
        Err(ChallengeMismatch)?
    }

    let message = authn_assertion_resp.message();
    let pk = VerifyingKey::from_sec1_bytes(public_key.0.as_ref())
        .map_err(PublicKeyConversionError)?;
    let sig =
        Signature::try_from(signature.as_ref()).map_err(SignatureConversionError)?;

    pk.verify(message.as_slice(), &sig)
        .map_err(|_| VerificationFailed)?;

    Ok(())
}

/// Parses the base64Url + DER encoded signature returned from
/// the passkey signer
pub fn parse_passkey_signature(signature: &str) -> Result<signature::P256> {
    let raw_der_formatted = base64::prelude::BASE64_URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(Base64DecodeError)?;
    let signature = p256::ecdsa::Signature::from_der(&raw_der_formatted)
        .map_err(|_| BadDerSignature)?;
    let sig = signature::P256(P256Signature::try_from(signature.as_ref())?);
    Ok(sig)
}

#[cfg(test)]
mod test {
    use crate::public_key::PublicKey;

    use super::*;

    #[test]
    fn authenticator_assertion_response_raw_json_round_trip() {
        let authenticator_data = "SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAAA";
        let client_data_json = "eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiT0RRMU5EQmxaREEwTm1Sak1EVmlaalpsTW1Sak16VTJZelJsWVdFelpqbGlORFJrWmpZNFl6UmpNVFppTjJZelkyRTRaalpqTjJWbU9EVTFPVEZsT1EiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQzMjEiLCJjcm9zc09yaWdpbiI6ZmFsc2V9";
        let auth_assertion_resp_raw = format!(
            r#"{{
                "authenticatorData": "{authenticator_data}",
                "clientDataJSON":"{client_data_json}",
                "signature":"MEYCIQCIiHJ4ivlVR0FEaLhtHtYRcYtwvs5tMc-GUVFWYxhkngIhAMcT5150L81hhyo0rb2MPU4QA4Urrixmq17SIvqq-INV",
                "userHandle":"0ekM_-HZhnhKgo298VOEAP867vpaMANMN_1hKztDSm4"
            }}"#
        );
        let deserialized: AuthenticatorAssertionResponseRaw =
            serde_json::from_str(&auth_assertion_resp_raw).unwrap();

        let expected = AuthenticatorAssertionResponseRaw {
            authenticator_data: base64::prelude::BASE64_URL_SAFE_NO_PAD
                .decode(authenticator_data)
                .unwrap(),
            client_data_json: base64::prelude::BASE64_URL_SAFE_NO_PAD
                .decode(client_data_json)
                .unwrap(),
        };

        assert_eq!(expected, deserialized);

        let serialized = serde_json::to_value(&deserialized).unwrap();
        assert_eq!(
            serde_json::json!({
                "authenticatorData": authenticator_data,
                "clientDataJSON":client_data_json,
            }),
            serialized
        )
    }

    #[test]
    fn get_challenge() {
        let operation_hash = "ODQ1NDBlZDA0NmRjMDViZjZlMmRjMzU2YzRlYWEzZjliNDRkZjY4YzRjMTZiN2YzY2E4ZjZjN2VmODU1OTFlOQ";
        let auth_assertion_resp_raw = r#"{
                "authenticatorData": "SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAAA",
                "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiT0RRMU5EQmxaREEwTm1Sak1EVmlaalpsTW1Sak16VTJZelJsWVdFelpqbGlORFJrWmpZNFl6UmpNVFppTjJZelkyRTRaalpqTjJWbU9EVTFPVEZsT1EiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQzMjEiLCJjcm9zc09yaWdpbiI6ZmFsc2V9",
                "signature":"MEYCIQCIiHJ4ivlVR0FEaLhtHtYRcYtwvs5tMc-GUVFWYxhkngIhAMcT5150L81hhyo0rb2MPU4QA4Urrixmq17SIvqq-INV",
                "userHandle":"0ekM_-HZhnhKgo298VOEAP867vpaMANMN_1hKztDSm4"
            }"#;

        let resp: AuthenticatorAssertionResponseRaw =
            serde_json::from_str(auth_assertion_resp_raw).unwrap();

        assert_eq!(operation_hash, resp.challenge_base64url().unwrap())
    }

    #[test]
    fn verify_passkey_test() {
        let signature = parse_passkey_signature("MEUCIQDv38zGXtPOEc3vO0SVloXyH2ipxd2ACyyDr1HlwrRCHgIgeYcrdOvoPm8nY_jhjtKbqJwVNrGYaf6Yv0l0EKAmNNk").unwrap();
        let public_key = match PublicKey::from_base58(
            "p2pk66MGWLuippApsduRsaN58P1dkVrAqDxBeSKFR4164Kx22uBmzTV",
        )
        .unwrap()
        {
            PublicKey::P256(p256) => p256,
            _ => panic!("Wrong key"),
        };
        let operation_hash =
            "84540ed046dc05bf6e2dc356c4eaa3f9b44df68c4c16b7f3ca8f6c7ef85591e9";
        let raw_operation_hash = hex::decode(operation_hash).unwrap();
        let auth_assertion_resp_raw = r#"{
                "authenticatorData": "SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAAA",
                "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiT0RRMU5EQmxaREEwTm1Sak1EVmlaalpsTW1Sak16VTJZelJsWVdFelpqbGlORFJrWmpZNFl6UmpNVFppTjJZelkyRTRaalpqTjJWbU9EVTFPVEZsT1EiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQzMjEiLCJjcm9zc09yaWdpbiI6ZmFsc2UsIm90aGVyX2tleXNfY2FuX2JlX2FkZGVkX2hlcmUiOiJkbyBub3QgY29tcGFyZSBjbGllbnREYXRhSlNPTiBhZ2FpbnN0IGEgdGVtcGxhdGUuIFNlZSBodHRwczovL2dvby5nbC95YWJQZXgifQ"
            }"#;

        let resp: AuthenticatorAssertionResponseRaw =
            serde_json::from_str(auth_assertion_resp_raw).unwrap();

        assert!(
            verify_passkey(&resp, &public_key, &signature, &raw_operation_hash).is_ok()
        );

        // Verify with invalid PK should fail verification
        let invalid_pk = match PublicKey::from_base58(
            "p2pk64gVWa253wwgKUsPzm9JzKLj5A1w5ypMRWvih8fsBoVCRHwMRkD",
        )
        .unwrap()
        {
            PublicKey::P256(p256) => p256,
            _ => panic!("Wrong key"),
        };
        let err = verify_passkey(&resp, &invalid_pk, &signature, &raw_operation_hash)
            .expect_err("Expected verification failure");
        assert!(matches!(
            err,
            crate::Error::PasskeyError {
                source: super::PasskeyError::VerificationFailed
            }
        ));
    }

    #[test]
    fn parse_passkey_signature_test() {
        let signature = parse_passkey_signature("MEUCIQDv38zGXtPOEc3vO0SVloXyH2ipxd2ACyyDr1HlwrRCHgIgeYcrdOvoPm8nY_jhjtKbqJwVNrGYaf6Yv0l0EKAmNNk").unwrap();
        let expected = "p2sigtghDmmBqGocWksbS78H4GeEjcahkYMabd5on2Sur9vMbJ1oTwAdpmTTVq4tJhLPLbiPvkb3N821bp7UZ7szjcJLF46uZJ";
        assert_eq!(expected, signature.to_base58_check());
    }
}
