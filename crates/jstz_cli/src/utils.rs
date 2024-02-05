use std::fs;
use std::io::{self, IsTerminal};

use crate::error::Result;

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
