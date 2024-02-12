use crate::{config::NetworkName, error::Result, utils::AddressOrAlias, Config};
use jstz_node::logs::QueryParams;

pub async fn exec(
    address_or_alias: AddressOrAlias,
    request_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    network: &Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load()?;

    let address = address_or_alias.resolve(&cfg)?;

    let query = if let Some(request_id) = request_id {
        // TODO: check if request_id is valid Blake2b?
        QueryParams::GetLogsByAddressAndRequestId(address, request_id)
    } else {
        QueryParams::GetLogsByAddress(address, limit, offset)
    };

    let res = cfg.jstz_client(network)?.logs_persistnet(query).await?;

    println!("\nRES: {:?}\n", res);

    Ok(())
}
