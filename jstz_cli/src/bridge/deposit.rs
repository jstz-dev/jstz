use anyhow::Result;
use bs58;

use crate::{config::Config, octez::OctezClient};

pub fn exec(from: String, to: String, amount: u64, cfg: &Config) -> Result<()> {
    // 1. Convert tz4 address to hexencoded bytes
    let to_bytes = bs58::decode(&to).into_vec().unwrap();
    let to_bytes = &to_bytes[3..to_bytes.len() - 4]; // Skip the first 3 bytes and the last 4 bytes
    let to_hex = hex::encode_upper(to_bytes);

    // 2. Execute the octez-client command
    OctezClient::transfer(
        cfg,
        &from,
        "jstz_bridge",
        "deposit",
        &format!("(Pair {} 0x{})", amount, to_hex),
    )
}
