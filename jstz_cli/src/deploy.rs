use anyhow::{anyhow, Result};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::{
    operation::{Content, DeployContract, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};

use crate::{
    account::account::Account,
    config::Config,
    jstz::JstzClient,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub async fn exec(
    self_address: Option<String>,
    contract_code: Option<String>,
    balance: u64,
    name: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    // Check if account already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) {
            return Err(anyhow!("Account already exists"));
        }
    }

    let account = cfg.accounts.account_or_current_mut(self_address)?;

    // Check if account is Owned
    let (nonce,
        _alias,
        address,
        secret_key,
        public_key,
        function_code) = match account {
        Account::Owned {
            nonce,
            alias,
            address,
            secret_key,
            public_key,
            function_code } => (nonce, address.clone(), alias.clone(), secret_key.clone(), public_key.clone(), function_code.clone()),
        _ => return Err(anyhow!("The account is an alias and cannot be used for deployment. Please use an owned account.")),
    };

    // resolve contract code
    let mut resolved_contract_code =
        contract_code.map(from_file_or_id).or_else(piped_input);

    match (&resolved_contract_code, &function_code) {
        (Some(_), Some(_)) => {
            return Err(anyhow!("You cannot supply a function code if the account already has a code set."));
        }
        (None, Some(func_code)) => {
            resolved_contract_code = Some(func_code.clone());
        }
        (None, None) => {
            return Err(anyhow!("No code supplied."));
        }
        _ => {}
    }

    // Create operation TODO nonce
    let op = Operation {
        source: PublicKeyHash::from_base58(&address.as_str())?,
        nonce: nonce.clone(),
        content: Content::DeployContract(DeployContract {
            contract_code: resolved_contract_code.ok_or(anyhow!("No code supplied."))?,
            contract_credit: balance,
        }),
    };

    nonce.increment();

    let signed_op = SignedOperation::new(public_key, secret_key.sign(op.hash())?, op);

    let hash = signed_op.hash();

    println!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // Send message to jstz
    OctezClient::send_rollup_external_message(
        cfg,
        "bootstrap2",
        bincode::serialize(&signed_op)?,
    )?;

    let receipt = JstzClient::new(cfg)
        .wait_for_operation_receipt(&hash)
        .await?;

    println!("Receipt: {:?}", receipt);

    // Create alias
    if let Some(name) = name {
        cfg.accounts.add_alias(
            name,
            match receipt.inner {
                Ok(ReceiptContent::DeployContract(deploy)) => {
                    (&deploy.contract_address).to_string()
                }
                _ => return Err(anyhow!("Content is not of type 'DeployContract'")),
            },
        )?;
    }

    cfg.save()?;

    Ok(())
}
