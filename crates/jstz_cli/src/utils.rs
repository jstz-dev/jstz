use crate::{
    config::{Config, NetworkName},
    error::{self, user_error, Error, Result},
};
use anyhow::anyhow;
use derive_more::Display;
use jstz_proto::context::account::Address;
use rust_decimal::Decimal;
use std::{
    fmt, fs,
    io::{self, IsTerminal},
    ops::Deref,
    str::FromStr,
};
use tezos_crypto_rs::hash::ContractKt1Hash;

const JSTZ_ADDRESS_PREFIXES: [&str; 4] = ["tz1", "tz2", "tz3", "KT1"];

const TEZ_DECIMALS: u32 = 6;
// 1 tez = 1,000,000 mutez
pub const MUTEZ_PER_TEZ: u64 = 10_u64.pow(TEZ_DECIMALS);

#[derive(Clone, Debug)]
pub enum AddressOrAlias {
    Address(Address),
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
                    .parse::<Address>()
                    .map_err(|e| anyhow!("{}", e))?,
            ))
        } else {
            Ok(Self::Alias(address_or_alias.to_string()))
        }
    }
}

impl fmt::Display for AddressOrAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(address) => write!(f, "{address}"),
            Self::Alias(alias) => write!(f, "{alias}"),
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

#[derive(Debug, Clone, Copy)]
pub struct Tez(Decimal);

impl TryFrom<Decimal> for Tez {
    type Error = error::Error;

    fn try_from(value: Decimal) -> Result<Self> {
        if value <= Decimal::ZERO {
            return Err(anyhow!("Value must be positive (greater than 0)"));
        }
        if value.scale() > TEZ_DECIMALS {
            return Err(anyhow!("Value has more than 6 digits of precision"));
        }
        Ok(Self(value))
    }
}

impl FromStr for Tez {
    type Err = error::Error;

    fn from_str(s: &str) -> Result<Self> {
        let decimal =
            Decimal::from_str(s).map_err(|e| anyhow!("Invalid decimal number: {}", e))?;
        Self::try_from(decimal)
    }
}

impl From<Tez> for Decimal {
    fn from(value: Tez) -> Self {
        value.0
    }
}

impl fmt::Display for Tez {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Tez {
    pub fn to_mutez(self) -> u64 {
        (self.0 * Decimal::from(MUTEZ_PER_TEZ))
            .round()
            .try_into()
            .unwrap()
    }
}

impl Deref for Tez {
    type Target = Decimal;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
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
            AddressOrAlias::Address(Address::SmartFunction(_))
        ));

        // Test valid tz1 address
        let result = AddressOrAlias::from_str(TEST_TZ1).unwrap();
        assert!(matches!(result, AddressOrAlias::Address(Address::User(_))));

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

    #[test]
    fn test_tez_amount_validation() {
        // Test positive values
        assert!(Tez::try_from(Decimal::from_str("1.0").unwrap()).is_ok());
        assert!(Tez::try_from(Decimal::from_str("0.000001").unwrap()).is_ok());

        // Test zero and negative values
        assert!(Tez::try_from(Decimal::ZERO).is_err());
        assert!(Tez::try_from(Decimal::from_str("-1.0").unwrap()).is_err());

        // Test precision limits
        assert!(Tez::try_from(Decimal::from_str("1.000000").unwrap()).is_ok());
        assert!(Tez::try_from(Decimal::from_str("1.0000001").unwrap()).is_err());
    }

    #[test]
    fn test_tez_amount_from_str() {
        // Valid cases
        assert!(Tez::from_str("1.0").is_ok());
        assert!(Tez::from_str("0.000001").is_ok());

        // Invalid cases
        assert!(Tez::from_str("0").is_err());
        assert!(Tez::from_str("-1.0").is_err());
        assert!(Tez::from_str("1.0000001").is_err());
        assert!(Tez::from_str("not_a_number").is_err());
    }

    #[test]
    fn test_tez_amount_conversions() {
        // Test Decimal conversion
        let decimal = Decimal::from_str("1.5").unwrap();
        let tez = Tez::try_from(decimal).unwrap();
        assert_eq!(Decimal::from(tez), decimal);

        // Test mutez conversion
        let tez = Tez::from_str("1.0").unwrap();
        assert_eq!(tez.to_mutez(), 1_000_000);

        let tez = Tez::from_str("0.000001").unwrap();
        assert_eq!(tez.to_mutez(), 1);

        let tez = Tez::from_str("1.5").unwrap();
        assert_eq!(tez.to_mutez(), 1_500_000);
    }
}
