use jstz_crypto::hash::Hash;
use log::{debug, info};

use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    sandbox::SANDBOX_BOOTSTRAP_ACCOUNTS,
    term::styles,
    utils::AddressOrAlias,
};

pub async fn exec(
    from: String,
    to: AddressOrAlias,
    amount: u64,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;

    // Check network
    if cfg.network_name(&network)? == NetworkName::Dev && cfg.sandbox.is_none() {
        bail_user_error!(
            "No sandbox is currently running. Please run {}.",
            styles::command("jstz sandbox start")
        );
    }

    let to_pkh = to.resolve(&cfg)?;

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
    let pkh = to_pkh.to_base58();
    debug!("resolved `to` -> {}", &pkh);

    // Execute the octez-client command
    if cfg
        .octez_client(&network)?
        .call_contract(
            &from,
            "jstz_native_bridge",
            "deposit",
            &format!("\"{}\"", &pkh),
            amount,
        )
        .is_err()
    {
        bail_user_error!("Failed to deposit XTZ. Please check whether the addresses and network are correct.");
    }

    info!(
        "Deposited {} XTZ from {} to {}",
        amount,
        from,
        to.to_string()
    );

    Ok(())
}
