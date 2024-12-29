use crate::{
    bridge::convert_tez_to_mutez,
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    run,
    term::styles,
    utils::AddressOrAlias,
};
use jstz_crypto::hash::JstzHash;
use log::debug;

pub async fn exec(
    to: AddressOrAlias,
    amount: f64,
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

    let to_pkh = to.resolve_l1(&cfg, &network)?;
    debug!("resolved `to` -> {}", &to_pkh.to_base58());

    let amount = convert_tez_to_mutez(amount)?;
    let url = "tezos://jstz/withdraw".to_string();
    let http_method = "POST".to_string();
    let gas_limit = 10; // TODO: set proper gas limit
    let withdraw = jstz_proto::executor::withdraw::Withdrawal {
        amount,
        receiver: to_pkh,
    };
    let json_data = serde_json::to_string(&withdraw)?;
    run::exec(url, http_method, gas_limit, Some(json_data), network, false).await
}
