use jstz_proto::context::account::Addressable;
use log::{debug, info};

use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    sandbox::{JSTZD_SERVER_BASE_URL, SANDBOX_BOOTSTRAP_ACCOUNTS},
    term::styles,
    utils::AddressOrAlias,
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
        .into_iter()
        .find(|address| *address == to_pkh.to_string())
    {
        bail_user_error!(
            "Cannot deposit to the bootstrap account '{}'.",
            bootstrap_account
        );
    }
    let pkh = to_pkh.to_base58();
    debug!("resolved `to` -> {}", &pkh);

    if cfg.sandbox().is_ok_and(|c| c.container) {
        let client = reqwest::Client::new();
        let res = client
            .post(format!("{JSTZD_SERVER_BASE_URL}/contract_call"))
            .json(&serde_json::json!({
                "from": from,
                "contract": NATIVE_BRIDGE_ADDRESS,
                "amount": amount,
                "entrypoint": "deposit",
                "arg": format!("\"{pkh}\"")
            }))
            .send()
            .await?;
        if !res.status().is_success() {
            bail_user_error!("Failed to deposit XTZ. Please check whether the addresses and network are correct.");
        }
    } else {
        // Execute the octez-client command
        if cfg
            .octez_client(&network)?
            .call_contract(
                &from,
                NATIVE_BRIDGE_ADDRESS,
                "deposit",
                &format!("\"{}\"", &pkh),
                amount,
            )
            .is_err()
        {
            bail_user_error!("Failed to deposit XTZ. Please check whether the addresses and network are correct.");
        }
    }

    info!(
        "Deposited {} XTZ from {} to {}",
        amount,
        from,
        to.to_string()
    );

    Ok(())
}
