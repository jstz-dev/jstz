use crate::{config::Config, error::Result, utils::AddressOrAlias};

pub fn exec(from: String, to: AddressOrAlias, amount: u64) -> Result<()> {
    let cfg = Config::load()?;
    let to = to.resolve(&cfg)?;

    // 2. Execute the octez-client command
    cfg.octez_client()?.call_contract(
        &from,
        "jstz_bridge",
        "deposit",
        &format!("(Pair {} 0x{})", amount, hex::encode_upper(to.as_bytes())),
    )?;

    println!("Deposited {} CTEZ to {}", amount, to);

    Ok(())
}
