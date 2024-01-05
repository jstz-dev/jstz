use std::fs;
use std::io::{self, IsTerminal};

pub fn from_file_or_id(data_or_file: String) -> String {
    // try and read the file
    fs::read_to_string(&data_or_file)
        // file doesn't exist so assume it's raw data
        .unwrap_or(data_or_file)
}

fn get_stdin() -> String {
    let lines: Result<Vec<_>, _> = io::stdin().lines().collect();
    lines.expect("Can't read from stdin").join("\n")
}

pub fn piped_input() -> Option<String> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        Some(get_stdin())
    } else {
        None
    }
}
