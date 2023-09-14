use std::process::Command;
use bs58;

pub fn deposit(from: String, to: String, amount: u64, octez_client_path: String, octez_client_setup_args: Vec<String>) {
    // Convert tz4 address to hexencoded bytes
    let to_bytes = bs58::decode(&to).into_vec().unwrap();
    let to_bytes = &to_bytes[3..to_bytes.len() - 4]; // Skip the first 3 bytes and the last 4 bytes
    let to_hex = hex::encode_upper(to_bytes); 


    let additional_args = [
        "transfer",
        "0",
        "from",
        &from,
        "to",
        "jstz_bridge",
        "--entrypoint",
        "deposit",
        "--arg",
        &format!("(Pair {} 0x{})", amount, to_hex),
        "--burn-cap",
        "999"
    ];

    let args: Vec<&str> = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
            additional_args.iter().cloned()
        )
        .collect();

    // Execute the octez-client command
    let output = Command::new(&octez_client_path)
        .args(&args)
        .output();

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