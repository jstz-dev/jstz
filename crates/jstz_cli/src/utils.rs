use std::{
    fs,
    io::{self, IsTerminal},
    str::FromStr,
};

use jstz_proto::context::account::Address;

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub enum AddressOrAlias {
    Address(Address),
    Alias(String),
}

impl AddressOrAlias {
    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias(_))
    }
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

// Function to get name of the file from string. If not a file returns None. Eg. /path/to/myfile.txt -> myfile
pub fn get_file_name_from_path(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    if path.is_file() {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        let file_name = file_name.split('.').next().unwrap();
        Some(file_name.to_string())
    } else {
        None
    }
}
