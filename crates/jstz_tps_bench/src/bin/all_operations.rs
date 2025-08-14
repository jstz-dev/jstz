use std::path::Path;

use anyhow::Context;
use http::{HeaderMap, Method, Uri};
use jstz_proto::{runtime::ParsedCode, HttpBody};
use jstz_utils::inbox_builder::InboxBuilder;

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
}

fn main() -> jstz_tps_bench::Result<()> {
    let args = Args::parse();
    let rollup_addr = SmartRollupAddress::from_b58check(&args.rollup_address)
        .context("failed to parse rollup address")?;
    let ticketer_addr = ContractKt1Hash::from_base58_check(&args.ticketer_address)
        .context("failed to parse ticketer address")?;
    let mut builder = InboxBuilder::new(rollup_addr, Some(ticketer_addr));
    let mut accounts = builder.create_accounts(2)?;

    builder.deposit_from_l1(&accounts[0], 1000000)?;

    // a regular smart function that refunds half of the amount received.
    let small_function_addr = builder.deploy_function(
        &mut accounts[0],
        ParsedCode(
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
        ),
        0,
    )?;

    // a large smart function that calls the refund smart function with the full amount received.
    let large_function_addr = builder.deploy_function(
        &mut accounts[0],
        ParsedCode(format!(
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
        )),
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

    let inbox = builder.build();
    inbox.save(&args.inbox_file)
}
