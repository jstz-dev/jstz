// SPDX-FileCopyrightText: 2024 TriliTech <contact@trili.tech>
//
// SPDX-License-Identifier: MIT

use std::path::Path;

use base64::{engine::general_purpose::URL_SAFE, Engine};
use http::{HeaderMap, Method, Uri};
use jstz_proto::context::account::{Address, Addressable};
use jstz_proto::HttpBody;
use serde::{Serialize, Serializer};
use tezos_smart_rollup::types::SmartRollupAddress;
use tezos_smart_rollup::utils::inbox::file::InboxFile;

use jstz_utils::inbox_builder::{Account, InboxBuilder, Result};

const FA2: &str = include_str!("../fa2/dist/index.js");

/// Generate the requested 'FA2 transfers', writing to `./inbox.json`.
///
/// This includes setup (contract deployment/minting) as well as balance checks at the end.
/// The transfers are generated with a 'follow on' strategy. For example 'account 0' will
/// have `num_accounts` minted of 'token 0'. It will then transfer all of them to 'account 1',
/// which will transfer `num_accounts - 1` to the next account, etc.
pub fn handle_generate(
    rollup_addr: &str,
    inbox_file: &Path,
    transfers: usize,
) -> Result<()> {
    let inbox = generate_inbox(rollup_addr, transfers)?;
    inbox.save(inbox_file)?;
    Ok(())
}

/// Like [`handle_generate`] but writes the inbox as a shell script.
pub fn handle_generate_script(
    rollup_addr: &str,
    script_file: &Path,
    transfers: usize,
) -> Result<()> {
    let inbox = generate_inbox(rollup_addr, transfers)?;
    inbox.save_script(script_file)?;
    Ok(())
}

/// Generate the inbox for the given rollup address and number of transfers.
fn generate_inbox(rollup_addr: &str, transfers: usize) -> Result<InboxFile> {
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

    let fa2_address = builder.deploy_function(&mut accounts[0], FA2.into(), 0)?;

    batch_mint(&mut builder, &mut accounts, &fa2_address)?;
    transfer(&mut builder, &mut accounts, &fa2_address, transfers)?;
    check_balance(&mut builder, &mut accounts, &fa2_address)?;

    Ok(builder.build())
}

#[derive(Debug, Serialize)]
struct MintNew<'a> {
    token_id: usize,
    #[serde(serialize_with = "address_ser")]
    owner: &'a Address,
    amount: usize,
}

#[derive(Debug, Serialize)]
struct BalanceRequest<'a> {
    token_id: usize,
    #[serde(serialize_with = "address_ser")]
    owner: &'a Address,
}

#[derive(Debug, Serialize)]
struct Transfer {
    token_id: usize,
    amount: usize,
    #[serde(serialize_with = "address_ser")]
    to: Address,
}

#[derive(Debug, Serialize)]
struct TransferToken<'a> {
    #[serde(serialize_with = "address_ser")]
    from: &'a Address,
    transfers: &'a [&'a Transfer],
}

fn address_ser<S>(address: &Address, ser: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let address = address.to_base58();
    String::serialize(&address, ser)
}

fn transfer(
    builder: &mut InboxBuilder,
    accounts: &mut [Account],
    fa2_address: &Address,
    transfers: usize,
) -> Result<()> {
    let len = accounts.len();
    let expected_len = builder.message_count() + transfers;

    'outer: for token_id in 0..len {
        for (from, amount) in (token_id..(token_id + len)).zip(1..len) {
            if expected_len == builder.message_count() {
                break 'outer;
            }

            let to = accounts[(from + 1) % len].address.clone();
            let transfer = Transfer {
                token_id,
                amount: len - amount,
                to,
            };

            let account = &mut accounts[from % len];
            let transfer = [TransferToken {
                from: &account.address,
                transfers: &[&transfer],
            }];

            let body = HttpBody::from_json(serde_json::to_value(&transfer)?);
            builder.run_function(
                account,
                Uri::try_from(format!("jstz://{fa2_address}/transfer"))?,
                Method::POST,
                HeaderMap::default(),
                body,
            )?;
        }
    }
    Ok(())
}

fn check_balance(
    builder: &mut InboxBuilder,
    accounts: &mut [Account],
    fa2_address: &Address,
) -> Result<()> {
    let tokens = 0..accounts.len();
    for account in accounts.iter_mut() {
        let reqs: Vec<_> = tokens
            .clone()
            .map(|i| BalanceRequest {
                owner: &account.address,
                token_id: i,
            })
            .collect();
        let query = serde_json::ser::to_vec(&reqs)?;
        let query = URL_SAFE.encode(query);

        builder.run_function(
            account,
            Uri::try_from(format!("jstz://{fa2_address}/balance_of?requests={query}"))?,
            Method::GET,
            HeaderMap::default(),
            HttpBody::empty(),
        )?;
    }
    Ok(())
}

fn batch_mint(
    builder: &mut InboxBuilder,
    accounts: &mut [Account],
    fa2_address: &Address,
) -> Result<()> {
    let amount = accounts.len() + 1;
    let mints: Vec<_> = accounts
        .iter()
        .enumerate()
        .map(|(i, a)| MintNew {
            token_id: i,
            owner: &a.address,
            amount,
        })
        .collect();

    let body = HttpBody::from_json(serde_json::to_value(&mints)?);
    builder.run_function(
        &mut accounts[0],
        Uri::try_from(format!("jstz://{fa2_address}/mint_new"))?,
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
