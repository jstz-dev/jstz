// SPDX-FileCopyrightText: 2024 TriliTech <contact@trili.tech>
//
// SPDX-License-Identifier: MIT

use std::path::Path;

use http::{HeaderMap, Method, Uri};
use jstz_proto::context::account::Address;
use jstz_proto::HttpBody;
use tezos_smart_rollup::types::SmartRollupAddress;
use tezos_smart_rollup::utils::inbox::file::InboxFile;

use jstz_utils::inbox_builder::{Account, InboxBuilder, Result};

/// Generate the requested transactions from the given contract, writing to `./inbox.json`.
///
/// This includes contract deployment, initialisation, requested transactions, and checks at the end.
pub fn handle_generate_other(
    rollup_addr: &str,
    inbox_file: &Path,
    transfers: usize,
    contract_file: &Path,
    init_endpoint: Option<&str>,
    transfer_endpoint: Option<&str>,
    check_endpoint: Option<&str>,
) -> Result<()> {
    let inbox = generate_inbox(
        rollup_addr,
        transfers,
        contract_file,
        init_endpoint,
        transfer_endpoint,
        check_endpoint,
    )?;
    inbox.save(inbox_file)?;
    Ok(())
}

/// Generate the inbox for the given rollup address and number of transfers.
fn generate_inbox(
    rollup_addr: &str,
    transfers: usize,
    contract_file: &Path,
    init_endpoint: Option<&str>,
    transfer_endpoint: Option<&str>,
    check_endpoint: Option<&str>,
) -> Result<InboxFile> {
    let rollup_addr = SmartRollupAddress::from_b58check(rollup_addr)?;

    let accounts = accounts_for_transfers(transfers);

    if accounts == 0 {
        return Err("--transfers must be greater than zero".into());
    }

    let mut builder = InboxBuilder::new(
        rollup_addr,
        None,
        #[cfg(feature = "v2_runtime")]
        None,
    );
    let mut accounts = builder.create_accounts(accounts)?;

    // Load the contract code from the given file
    let code_string = std::fs::read_to_string(contract_file)?;

    let contract_address = builder.deploy_function(&mut accounts[0], code_string, 0)?;

    init(
        &mut builder,
        &mut accounts[0],
        &contract_address,
        init_endpoint,
    )?;
    transfer(
        &mut builder,
        &mut accounts[0],
        &contract_address,
        transfers,
        transfer_endpoint,
    )?;
    check(
        &mut builder,
        &mut accounts[0],
        &contract_address,
        check_endpoint,
    )?;

    Ok(builder.build())
}

fn transfer(
    builder: &mut InboxBuilder,
    account: &mut Account,
    contract_address: &Address,
    transfers: usize,
    transfer_endpoint: Option<&str>,
) -> Result<()> {
    let endpoint = transfer_endpoint.unwrap_or("transfer");
    let transfer_endpoint_uri =
        Uri::try_from(format!("jstz://{contract_address}/{endpoint}"))?;

    for _ in 0..transfers {
        builder.run_function(
            account,
            transfer_endpoint_uri.clone(),
            Method::POST,
            HeaderMap::default(),
            HttpBody::empty(),
        )?;
    }

    Ok(())
}

// Simple check after all transactions have been run
fn check(
    builder: &mut InboxBuilder,
    account: &mut Account,
    contract_address: &Address,
    check_endpoint: Option<&str>,
) -> Result<()> {
    let endpoint = check_endpoint.unwrap_or("check");
    let check_endpoint_uri =
        Uri::try_from(format!("jstz://{contract_address}/{endpoint}"))?;

    builder.run_function(
        account,
        check_endpoint_uri,
        Method::GET,
        HeaderMap::default(),
        HttpBody::empty(),
    )?;
    Ok(())
}

// Initialise the contract
fn init(
    builder: &mut InboxBuilder,
    account: &mut Account,
    contract_address: &Address,
    init_endpoint: Option<&str>,
) -> Result<()> {
    let endpoint = init_endpoint.unwrap_or("init");
    let init_endpoint_uri =
        Uri::try_from(format!("jstz://{contract_address}/{endpoint}"))?;

    let body = HttpBody::empty();
    builder.run_function(
        account,
        init_endpoint_uri,
        Method::POST,
        HeaderMap::default(),
        body,
    )
}

/// The generation strategy supports up to `num_accounts ^ 2` transfers,
/// find the smallest number of accounts which will allow for this.
fn accounts_for_transfers(transfers: usize) -> usize {
    f64::sqrt(transfers as f64).ceil() as usize + 1
}
