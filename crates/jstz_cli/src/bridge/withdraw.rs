use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    run::{self, RunArgs},
    sandbox::{assert_sandbox_running, JSTZD_SERVER_BASE_URL},
    utils::{AddressOrAlias, Tez},
};
use anyhow::Context;
use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
use jstz_proto::context::account::{Address, Addressable};
use log::debug;
use reqwest::StatusCode;

pub async fn exec(
    to: AddressOrAlias,
    amount: Tez,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;

    // Check network
    let receiver = if cfg.network_name(&network)? == NetworkName::Dev {
        assert_sandbox_running(JSTZD_SERVER_BASE_URL).await?;
        sandbox_resolve_l1(to, JSTZD_SERVER_BASE_URL).await?
    } else {
        to.resolve_l1(&cfg, &network).await?
    };

    debug!("resolved `to` -> {}", &receiver.to_base58());

    let amount = amount.to_mutez();
    let url = "jstz://jstz/withdraw".to_string();
    let http_method = "POST".to_string();
    let gas_limit = 10; // TODO: set proper gas limit
    let withdraw = jstz_proto::executor::withdraw::Withdrawal { amount, receiver };
    let json_data = serde_json::to_string(&withdraw)?;
    let args = RunArgs::new(url, http_method, gas_limit);
    run::exec(args.set_json_data(Some(json_data)).set_network(network)).await
}

async fn sandbox_resolve_l1(
    to: AddressOrAlias,
    jstzd_server_base_url: &str,
) -> Result<Address> {
    match to {
        AddressOrAlias::Address(v) => Ok(v),
        AddressOrAlias::Alias(alias) => {
            // go through jstzd server even when the sandbox is not in a container for simplicity
            let res = reqwest::get(format!("{jstzd_server_base_url}/l1_alias/{alias}"))
                .await
                .context("failed to connect to jstzd server")?;
            match res.status() {
                StatusCode::OK => Ok(Address::User(
                    PublicKeyHash::from_base58(
                        &res.text()
                            .await
                            .context("failed to load text from response")?,
                    )
                    .context("failed to parse address from response")?,
                )),
                StatusCode::NOT_FOUND => {
                    bail_user_error!("Unknown L1 address alias '{}'", alias)
                }
                _ => bail_user_error!(
                    "Failed to resolve L1 address aliases in the sandbox."
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sandbox_resolve_l1;
    use crate::utils::AddressOrAlias;
    use std::str::FromStr;

    #[tokio::test]
    async fn sandbox_resolve_l1_address() {
        assert_eq!(
            sandbox_resolve_l1(
                AddressOrAlias::from_str("tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV").unwrap(),
                ""
            )
            .await
            .unwrap()
            .to_string(),
            "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV"
        );
    }

    #[tokio::test]
    async fn sandbox_resolve_l1_alias_ok() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/l1_alias/foo")
            .with_body("tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV")
            .create();
        assert_eq!(
            sandbox_resolve_l1(AddressOrAlias::from_str("foo").unwrap(), &server.url())
                .await
                .unwrap()
                .to_string(),
            "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV"
        );
    }

    #[tokio::test]
    async fn sandbox_resolve_l1_alias_server_unreachable() {
        assert_eq!(
            sandbox_resolve_l1(AddressOrAlias::from_str("foo").unwrap(), "bad_url")
                .await
                .unwrap_err()
                .to_string(),
            "failed to connect to jstzd server"
        );
    }

    #[tokio::test]
    async fn sandbox_resolve_l1_alias_bad_response() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/l1_alias/foo")
            .with_body("foo")
            .create();
        assert_eq!(
            sandbox_resolve_l1(AddressOrAlias::from_str("foo").unwrap(), &server.url())
                .await
                .unwrap_err()
                .to_string(),
            "failed to parse address from response"
        );
    }

    #[tokio::test]
    async fn sandbox_resolve_l1_alias_not_found() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/l1_alias/foo")
            .with_status(404)
            .create();
        assert_eq!(
            sandbox_resolve_l1(AddressOrAlias::from_str("foo").unwrap(), &server.url())
                .await
                .unwrap_err()
                .to_string(),
            "Unknown L1 address alias 'foo'"
        );
    }

    #[tokio::test]
    async fn sandbox_resolve_l1_alias_err() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/l1_alias/foo")
            .with_status(500)
            .create();
        assert_eq!(
            sandbox_resolve_l1(AddressOrAlias::from_str("foo").unwrap(), &server.url())
                .await
                .unwrap_err()
                .to_string(),
            "Failed to resolve L1 address aliases in the sandbox."
        );
    }
}
