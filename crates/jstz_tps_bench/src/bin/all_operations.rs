use std::path::Path;

use anyhow::Context;
use http::{HeaderMap, Method, Uri};
use jstz_proto::{
    context::account::{Address, Nonce},
    runtime::v2::fetch::http::{Body, Response},
    HttpBody,
};
use jstz_utils::{
    inbox_builder::{Account, InboxBuilder},
    key_pair::{parse_key_file, KeyPair},
};

use clap::Parser;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::types::SmartRollupAddress;

#[derive(Parser, Debug)]
#[command(
    about = "Generates inbox messages for most of the operations available in Jstz."
)]
struct Args {
    /// Target rollup address.
    #[arg(long)]
    rollup_address: String,

    /// Jstz ticketer contract address.
    #[arg(long)]
    ticketer_address: String,

    /// Path to the output inbox file.
    #[arg(long, default_value = "inbox.json")]
    inbox_file: Box<Path>,

    /// Public-private key pair representing the oracle response signer. (format: {"public_key": ..., "secret_key": ...})
    #[arg(long)]
    oracle_key_file: Option<Box<Path>>,
}

fn main() -> jstz_tps_bench::Result<()> {
    let args = Args::parse();
    let rollup_addr = SmartRollupAddress::from_b58check(&args.rollup_address)
        .context("failed to parse rollup address")?;
    let ticketer_addr = ContractKt1Hash::from_base58_check(&args.ticketer_address)
        .context("failed to parse ticketer address")?;
    let oracle_signer = match args.oracle_key_file {
        Some(path) => {
            let KeyPair(pk, sk) = parse_key_file(path.to_path_buf())?;
            Some(Account {
                // FIXME: nonce needs to start from 1 because currently the oracle signer is also
                // the injector and there is one large payload operation before the oracle call,
                // which means by the time the signer gets its first task, its nonce is already 1.
                nonce: Nonce(1),
                address: Address::from_base58(&pk.hash())?,
                sk,
                pk,
            })
        }
        None => None,
    };
    let mut builder = InboxBuilder::new(rollup_addr, Some(ticketer_addr), oracle_signer);
    let mut accounts = builder.create_accounts(2)?;

    builder.deposit_from_l1(&accounts[0], 1000000)?;

    // a regular smart function that refunds half of the amount received.
    let small_function_addr = builder.deploy_function(
        &mut accounts[0],
        r#"
export default (request) => {
    const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
    console.log("small function transferred amount", transferred_amount);
    return new Response(null, {
        headers: {
            "X-JSTZ-TRANSFER": `${transferred_amount / 2}`,
        },
    });
};"#
        .to_string(),
        0,
    )?;

    // a large smart function that calls the refund smart function with the full amount received.
    let large_function_addr = builder.deploy_function(
        &mut accounts[0],
        format!(
            r#"
export default async (request) => {{
    const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
    let long_string = "{}";
    console.log("large function transferred amount", transferred_amount);
    const call_request = new Request("jstz://{}/", {{
        headers: {{
            "X-JSTZ-TRANSFER": `${{transferred_amount}}`,
        }},
    }});
    return await fetch(call_request);
}};"#,
            "a".repeat(4096),
            small_function_addr,
        ),
        0,
    )?;

    // run the large smart function with 0.5 tez
    builder.run_function(
        &mut accounts[0],
        Uri::try_from(format!("jstz://{large_function_addr}/"))?,
        Method::GET,
        HeaderMap::from_iter([(
            "X-JSTZ-TRANSFER".parse().unwrap(),
            "500000".parse().unwrap(),
        )]),
        HttpBody::empty(),
    )?;

    // oracle
    let oracle_function_addr = builder.deploy_function(
        &mut accounts[0],
        r#"
export default async (request) => {
    const call_request = new Request("http://foo.bar/");
    const response = await fetch(call_request);
    console.log("oracle response status:", response.status);
    return response;
};"#
        .to_string(),
        0,
    )?;
    builder.run_function(
        &mut accounts[0],
        Uri::try_from(format!("jstz://{oracle_function_addr}/"))?,
        Method::GET,
        HeaderMap::default(),
        HttpBody::empty(),
    )?;

    builder.create_oracle_response(Response {
        status: 204,
        status_text: String::default(),
        headers: vec![],
        body: Body::zero_capacity(),
    })?;

    builder.bump_level()?;

    let receiver = accounts[0].address.clone();
    builder.withdraw(&mut accounts[0], &receiver, 1)?;

    let inbox = builder.build();
    inbox.save(&args.inbox_file)
}
