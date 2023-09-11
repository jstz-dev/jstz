use std::process::Command;
use bs58;

pub fn deposit(from: String, to: String, amount: u64) {
    // Convert tz4 address to hexencoded bytes
    let to_bytes = bs58::decode(&to).into_vec().unwrap();
    let to_bytes = &to_bytes[3..to_bytes.len() - 4]; // Skip the first 3 bytes and the last 4 bytes
    let to_hex = hex::encode_upper(to_bytes); 

    let root_dir = ".."; 
    let rpc=18730;

    // Construct the full path to the octez-client
    let octez_client_path = format!("{}/octez-client", root_dir);

    // Execute the octez-client command
    let output = Command::new(&octez_client_path)
        .arg("-base-dir")
        .arg("${OCTEZ_CLIENT_DIR}")
        .arg("-endpoint")
        .arg(format!("http://127.0.0.1:{}", rpc)) 
        .arg("transfer")
        .arg("0")
        .arg("from")
        .arg(&from)
        .arg("to")
        .arg("jstz_bridge")
        .arg("--entrypoint")
        .arg("deposit")
        .arg("--arg")
        .arg(format!("(Pair {} 0x{})", amount, to_hex))
        .arg("--burn-cap")
        .arg("999")
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