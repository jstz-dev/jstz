use log::{debug, info};

use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    sandbox::SANDBOX_BOOTSTRAP_ACCOUNTS,
    term::styles,
    utils::AddressOrAlias,
};

pub fn exec(
    from: String,
    to: AddressOrAlias,
    amount: u64,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load()?;

    // Check network
    if cfg.network_name(&network)? == NetworkName::Dev && cfg.sandbox.is_none() {
        bail_user_error!(
            "No sandbox is currently running. Please run {}.",
            styles::command("jstz sandbox start")
        );
    }

    let to_pkh = to.resolve(&cfg)?;
    debug!("resolved `to` -> {:?}", to_pkh);

    // Check if trying to deposit to a bootsrap account.
    if let Some(bootstrap_account) = SANDBOX_BOOTSTRAP_ACCOUNTS
        .iter()
        .find(|account| account.address == to_pkh.to_string())
    {
        bail_user_error!(
            "Cannot deposit to the bootstrap account '{}'.",
            bootstrap_account.address
        );
    }

    // Execute the octez-client command
    if let Err(_) = cfg.octez_client(&network)?.call_contract(
        &from,
        "jstz_bridge",
        "deposit",
        &format!(
            "(Pair {} 0x{})",
            amount,
            hex::encode_upper(to_pkh.as_bytes())
        ),
    ) {
        bail_user_error!("Failed to deposit CTEZ. Please check whether the addresses and network are correct.");
    }

    info!(
        "Deposited {} CTEZ from {} to {}",
        amount,
        from,
        to.to_string()
    );

    Ok(())
}
