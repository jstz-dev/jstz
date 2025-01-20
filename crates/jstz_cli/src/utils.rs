use crate::{
    config::{Config, NetworkName},
    error::{user_error, Error, Result},
};
use anyhow::anyhow;
use derive_more::Display;
use jstz_proto::context::new_account::NewAddress;
use std::{
    env,
    fmt::{self, Display},
    fs,
    io::{self, IsTerminal},
    str::FromStr,
};
use tezos_crypto_rs::hash::ContractKt1Hash;

const JSTZ_ADDRESS_PREFIXES: [&str; 4] = ["tz1", "tz2", "tz3", "KT1"];

#[derive(Clone, Debug)]
pub enum AddressOrAlias {
    Address(NewAddress),
    Alias(String),
}

impl FromStr for AddressOrAlias {
    type Err = Error;

    fn from_str(address_or_alias: &str) -> Result<Self> {
        let is_jstz_address = JSTZ_ADDRESS_PREFIXES
            .iter()
            .any(|prefix| address_or_alias.starts_with(prefix));

        if is_jstz_address {
            Ok(Self::Address(
                address_or_alias
                    .parse::<NewAddress>()
                    .map_err(|e| anyhow!("{}", e))?,
            ))
        } else {
            Ok(Self::Alias(address_or_alias.to_string()))
        }
    }
}

impl Display for AddressOrAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(address) => write!(f, "{}", address),
            Self::Alias(alias) => write!(f, "{}", alias),
        }
    }
}

#[derive(Clone, Debug, Display)]
pub enum OriginatedOrAlias {
    Address(ContractKt1Hash),
    Alias(String),
}

impl OriginatedOrAlias {
    pub fn resolve(
        &self,
        cfg: &Config,
        network: &Option<NetworkName>,
    ) -> Result<ContractKt1Hash> {
        match self {
            OriginatedOrAlias::Address(kt1) => Ok(kt1.clone()),
            OriginatedOrAlias::Alias(alias) => {
                let address = cfg.octez_client(network)?
                    .resolve_contract(alias).map_err(|_|user_error!(
                        "Alias '{}' not found in octez-client. Please provide a valid address or alias.",
                        alias
                    ))?;

                let address = ContractKt1Hash::from_base58_check(&address)?;
                Ok(address)
            }
        }
    }
}

impl FromStr for OriginatedOrAlias {
    type Err = Error;

    fn from_str(address_or_alias: &str) -> Result<Self> {
        if address_or_alias.starts_with("KT1") {
            Ok(Self::Address(address_or_alias.parse()?))
        } else {
            Ok(Self::Alias(address_or_alias.to_string()))
        }
    }
}

pub fn read_file_or_input(input_or_filename: String) -> String {
    // try and read the file
    fs::read_to_string(&input_or_filename)
        // file doesn't exist so assume it's raw data
        .unwrap_or(input_or_filename)
}

fn read_stdin_lines() -> Result<String> {
    let lines = io::stdin().lines().collect::<io::Result<Vec<_>>>()?;
    Ok(lines.join("\n"))
}

pub fn read_piped_input() -> Result<Option<String>> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        Ok(Some(read_stdin_lines()?))
    } else {
        Ok(None)
    }
}

pub fn read_file_or_input_or_piped(
    input_or_filename: Option<String>,
) -> Result<Option<String>> {
    let contents = input_or_filename.map(read_file_or_input);

    match contents {
        Some(x) => Ok(Some(x)),
        None => {
            // If none, read piped input
            read_piped_input()
        }
    }
}

pub fn using_jstzd() -> bool {
    matches!(
        env::var("USE_JSTZD").as_ref().map(String::as_str),
        Ok("true") | Ok("1")
    )
}

pub fn convert_tez_to_mutez(tez: f64) -> Result<u64> {
    // 1 XTZ = 1,000,000 Mutez
    let mutez = tez * 1_000_000.0;
    if mutez.fract() != 0. {
        Err(user_error!(
            "Invalid amount: XTZ can have at most 6 decimal places"
        ))?;
    }

    Ok(mutez as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KT1: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
    const TEST_TZ1: &str = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU";
    const TEST_ALIAS: &str = "my_contract";

    #[test]
    fn test_parse_address_or_alias() {
        // Test valid KT1 address
        let result = AddressOrAlias::from_str(TEST_KT1).unwrap();
        assert!(matches!(
            result,
            AddressOrAlias::Address(NewAddress::SmartFunction(_))
        ));

        // Test valid tz1 address
        let result = AddressOrAlias::from_str(TEST_TZ1).unwrap();
        assert!(matches!(
            result,
            AddressOrAlias::Address(NewAddress::User(_))
        ));

        // Test alias
        let result = AddressOrAlias::from_str(TEST_ALIAS).unwrap();
        assert!(matches!(
            result,
            AddressOrAlias::Alias(alias) if alias == TEST_ALIAS
        ));
    }

    #[test]
    fn test_parse_invalid_address() {
        // Test invalid address format
        let result = AddressOrAlias::from_str("KT1invalid");
        assert!(result.is_err());
    }
}
