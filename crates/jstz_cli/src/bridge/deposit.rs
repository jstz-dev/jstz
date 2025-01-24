use log::{debug, info};

use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    sandbox::SANDBOX_BOOTSTRAP_ACCOUNTS,
    term::styles,
    utils::{using_jstzd, AddressOrAlias},
};

// hardcoding it here instead of importing from jstzd simply to avoid adding jstzd
// as a new depedency of jstz_cli just for this so that build time remains the same
const NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

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

    let contract = match using_jstzd() || cfg.sandbox().is_ok_and(|c| c.container) {
        // Since jstz contracts are loaded as bootstrap contracts in jstzd,
        // octez-client does not recognise them by alias, but addresses
        // remain constant for bootstrap contracts, so we can use the KT1 address here
        true => NATIVE_BRIDGE_ADDRESS,
        _ => "jstz_native_bridge",
    };
    // Execute the octez-client command
    if cfg
        .octez_client(&network)?
        .call_contract(&from, contract, "deposit", &format!("\"{}\"", &pkh), amount)
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
