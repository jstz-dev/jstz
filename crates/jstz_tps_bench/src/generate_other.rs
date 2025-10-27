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

/// Generate the requested operations from the given contract, writing to `./inbox.json`.
///
/// This includes contract deployment, initialisation, requested operations, and checks at the end.
pub fn handle_generate_other(
    rollup_addr: &str,
    inbox_file: &Path,
    num_operations: usize,
    smart_function: &Path,
    init_endpoint: Option<&str>,
    run_endpoint: Option<&str>,
    check_endpoint: Option<&str>,
) -> Result<()> {
    let inbox = generate_inbox(
        rollup_addr,
        num_operations,
        smart_function,
        init_endpoint,
        run_endpoint,
        check_endpoint,
    )?;
    inbox.save(inbox_file)?;
    Ok(())
}

/// Prepare an `InboxBuilder` and a set of accounts for subsequent operations.
pub fn prepare_builder_and_accounts(
    rollup_addr: &str,
    num_operations: usize,
) -> Result<(InboxBuilder, Vec<Account>)> {
    if num_operations == 0 {
        return Err("Number of operations must be greater than zero".into());
    }

    let accounts = accounts_for_operations(num_operations);
    let rollup_addr = SmartRollupAddress::from_b58check(rollup_addr)?;

    let mut builder = InboxBuilder::new(
        rollup_addr,
        None,
        #[cfg(feature = "v2_runtime")]
        None,
    );
    let accounts = builder.create_accounts(accounts)?;

    Ok((builder, accounts))
}

/// Generate the inbox for the given rollup address and number of transfers.
fn generate_inbox(
    rollup_addr: &str,
    num_operations: usize,
    smart_function: &Path,
    init_endpoint: Option<&str>,
    run_endpoint: Option<&str>,
    check_endpoint: Option<&str>,
) -> Result<InboxFile> {
    let (mut builder, mut accounts) =
        prepare_builder_and_accounts(rollup_addr, num_operations)?;

    // Load the contract code from the given file
    let code_string = std::fs::read_to_string(smart_function)?;

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
        num_operations,
        run_endpoint,
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
    num_operations: usize,
    run_endpoint: Option<&str>,
) -> Result<()> {
    let endpoint = run_endpoint.unwrap_or("transfer");
    let run_endpoint_uri =
        Uri::try_from(format!("jstz://{contract_address}/{endpoint}"))?;

    for _ in 0..num_operations {
        builder.run_function(
            account,
            run_endpoint_uri.clone(),
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
fn accounts_for_operations(num_operations: usize) -> usize {
    f64::sqrt(num_operations as f64).ceil() as usize + 1
}
