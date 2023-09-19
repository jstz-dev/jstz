use bs58;
use crate::config::Config;
use crate::utils::handle_output;

pub fn deposit(from: String, to: String, amount: u64, cfg: &Config) {
    // Convert tz4 address to hexencoded bytes
    let to_bytes = bs58::decode(&to).into_vec().unwrap();
    let to_bytes = &to_bytes[3..to_bytes.len() - 4]; // Skip the first 3 bytes and the last 4 bytes
    let to_hex = hex::encode_upper(to_bytes); 

    // Execute the octez-client command
    let output = cfg.octez_client_command()
        .args(
            [
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
            ]
        )
        .output();

    handle_output(&output);
}