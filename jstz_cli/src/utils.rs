pub fn handle_output(output: &Result<std::process::Output, std::io::Error>) {
    match output {
        Ok(output) => {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            eprintln!("Error: Failed to execute the `octez-client` command.");
            eprintln!("Detailed error: {}", e);
            std::process::exit(1);
        }
    }
}
