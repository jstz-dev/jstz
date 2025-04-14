use std::ops::Deref;

use jstz_proto::context::account::Addressable;
use log::{debug, info};

use crate::{
    config::{Config, NetworkName},
    error::{bail_user_error, Result},
    sandbox::{
        assert_sandbox_running, JSTZD_SERVER_BASE_URL, SANDBOX_BOOTSTRAP_ACCOUNTS,
    },
    utils::{AddressOrAlias, Tez},
};

// hardcoding it here instead of importing from jstzd simply to avoid adding jstzd
// as a new depedency of jstz_cli just for this so that build time remains the same
const NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

pub async fn exec(
    from: String,
    to: AddressOrAlias,
    amount: Tez,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;
    let use_sandbox = cfg.network_name(&network)? == NetworkName::Dev;
    // Check network
    if use_sandbox {
        assert_sandbox_running(JSTZD_SERVER_BASE_URL).await?;
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
    if use_sandbox {
        exec_sandbox(JSTZD_SERVER_BASE_URL, &from, &pkh, amount).await?;
    } else {
        // Execute the octez-client command
        if cfg
            .octez_client(&network)?
            .call_contract(
                &from,
                NATIVE_BRIDGE_ADDRESS,
                "deposit",
                &format!("\"{}\"", &pkh),
                amount.deref(),
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

async fn exec_sandbox(
    jstzd_server_base_url: &str,
    from: &str,
    to_pkh: &str,
    amount: Tez,
) -> Result<()> {
    // go through jstzd server even when the sandbox is not in a container for simplicity
    let client = reqwest::Client::new();
    // TODO: Use `Tez` for amount
    // https://linear.app/tezos/issue/JSTZ-475/use-tez-or-decimals
    let amount: f64 = amount.to_string().parse().unwrap();
    let res = client
        .post(format!("{jstzd_server_base_url}/contract_call"))
        .json(&serde_json::json!({
            "from": from,
            "contract": NATIVE_BRIDGE_ADDRESS,
            "amount": amount,
            "entrypoint": "deposit",
            "arg": format!("\"{to_pkh}\"")
        }))
        .send()
        .await?;
    if !res.status().is_success() {
        bail_user_error!("Failed to deposit XTZ. Please check whether the addresses and network are correct.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::utils::Tez;

    use super::exec_sandbox;

    #[tokio::test]
    async fn exec_sandbox_ok() {
        let mut server = mockito::Server::new_async().await;
        server.mock("POST", "/contract_call").create();

        assert!(exec_sandbox(
            &server.url(),
            "",
            "",
            Tez::try_from(Decimal::from(1)).unwrap()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn exec_sandbox_failed_to_send_request() {
        assert_eq!(
            exec_sandbox("bad url", "", "", Tez::try_from(Decimal::from(1)).unwrap())
                .await
                .unwrap_err()
                .to_string(),
            "builder error: relative URL without a base"
        );
    }

    #[tokio::test]
    async fn exec_sandbox_bad_request() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/contract_call")
            .with_status(422)
            .create();

        assert_eq!(exec_sandbox(&server.url(), "", "", Tez::try_from(Decimal::from(1)).unwrap()).await.unwrap_err().to_string(), "Failed to deposit XTZ. Please check whether the addresses and network are correct.");
    }
}
