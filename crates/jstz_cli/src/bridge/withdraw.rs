use crate::{
    bridge::convert_tez_to_mutez,
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    run::{self, RunArgs},
    term::styles,
    utils::AddressOrAlias,
};
use jstz_proto::context::account::Addressable;
use log::debug;

pub async fn exec(
    to: AddressOrAlias,
    amount: f64,
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

    let receiver = to.resolve_l1(&cfg, &network)?;
    debug!("resolved `to` -> {}", &receiver.to_base58());

    let amount = convert_tez_to_mutez(amount)?;
    let url = "tezos://jstz/withdraw".to_string();
    let http_method = "POST".to_string();
    let gas_limit = 10; // TODO: set proper gas limit
    let withdraw = jstz_proto::executor::withdraw::Withdrawal { amount, receiver };
    let json_data = serde_json::to_string(&withdraw)?;
    let args = RunArgs::new(url, http_method, gas_limit);
    run::exec(args.set_json_data(Some(json_data)).set_network(network)).await
}
