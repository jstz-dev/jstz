use std::path::PathBuf;

use anyhow::Context;
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "PublicKey")]
pub struct KeyPair(pub PublicKey, pub SecretKey);

impl From<KeyPair> for PublicKey {
    fn from(value: KeyPair) -> Self {
        value.0
    }
}

#[derive(Debug, Deserialize)]
struct RawKeyPair {
    public_key: String,
    secret_key: String,
}

/// Parses a public-secret key pair from a JSON file. The JSON content must have two keys:
/// * `public_key`: with a public key string starting with `edpk`
/// * `secret_key`: with a secret key string starting with `edsk`
pub fn parse_key_file(path: PathBuf) -> anyhow::Result<KeyPair> {
    let key_pair = std::fs::read_to_string(path).context("Failed to read key file")?;
    let RawKeyPair {
        public_key,
        secret_key,
    } = serde_json::from_str(&key_pair).map_err(|_| {
        anyhow::anyhow!("Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields")
    })?;

    let public_key = PublicKey::from_base58(&public_key).context("Invalid public key")?;
    let secret_key = SecretKey::from_base58(&secret_key).context("Invalid secret key")?;

    Ok(KeyPair(public_key, secret_key))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Seek, Write},
        path::PathBuf,
        str::FromStr,
    };

    use super::KeyPair;
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use tempfile::NamedTempFile;

    #[test]
    fn parse_key_file() {
        assert_eq!(
            super::parse_key_file(PathBuf::from_str("/foo/bar").unwrap())
                .unwrap_err()
                .to_string(),
            "Failed to read key file"
        );

        let mut tmp_file = NamedTempFile::new().unwrap();
        tmp_file.write_all(b"a:b:c").unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ3",
  "secret_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Invalid public key"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
  "secret_key": "a"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
  "secret_key": "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        let KeyPair(public_key, secret_key) =
            super::parse_key_file(tmp_file.path().to_path_buf()).unwrap();
        assert_eq!(
            public_key,
            PublicKey::from_base58(
                "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
            )
            .unwrap()
        );
        assert_eq!(
            secret_key,
            SecretKey::from_base58(
                "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
            )
            .unwrap()
        );
    }
}
