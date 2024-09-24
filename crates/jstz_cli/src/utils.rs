use std::{
    fs,
    io::{self, IsTerminal},
    str::FromStr,
};

use jstz_proto::context::account::Address;
use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::error::{user_error, Error, Result};

#[derive(Clone, Debug)]
pub enum AddressOrAlias {
    Address(Address),
    Alias(String),
}

impl FromStr for AddressOrAlias {
    type Err = Error;

    fn from_str(address_or_alias: &str) -> Result<Self> {
        if address_or_alias.starts_with("tz1") {
            Ok(Self::Address(address_or_alias.parse()?))
        } else {
            Ok(Self::Alias(address_or_alias.to_string()))
        }
    }
}

impl ToString for AddressOrAlias {
    fn to_string(&self) -> String {
        match self {
            Self::Address(address) => address.to_string(),
            Self::Alias(alias) => alias.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum OriginatedOrAlias {
    Address(ContractKt1Hash),
    Alias(String),
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

impl ToString for OriginatedOrAlias {
    fn to_string(&self) -> String {
        match self {
            OriginatedOrAlias::Address(contract) => contract.to_base58_check(),
            OriginatedOrAlias::Alias(alias) => alias.clone(),
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
