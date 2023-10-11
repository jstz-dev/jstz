use boa_engine::Source;
use jstz_core::runtime;
use jstz_core::runtime::Runtime;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use tezos_smart_rollup_mock::MockHost;

use jstz_core::host::HostRuntime;

use jstz_api::http::HttpApi;
use jstz_api::url::UrlApi;
use jstz_api::ConsoleApi;
use jstz_api::KvApi;
use jstz_api::TextEncoderApi;

use jstz_proto::api::ContractApi;
use jstz_proto::api::LedgerApi;

use jstz_crypto::public_key_hash::PublicKeyHash;

use anyhow::Result;

pub fn exec(self_address: Option<String>) -> Result<()> {
    let mock_address_string = "tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q";

    let contract_address_string = if let Some(address) = self_address {
        println!("Self address set to {}.", address);
        address
    } else {
        println!("Using mock self-address {}.", mock_address_string);
        mock_address_string.to_string()
    };

    let contract_address = PublicKeyHash::from_base58(contract_address_string.as_str())
        .expect("Failed to create contract address.");

    let mut rt = Runtime::new().expect("Failed to create a new runtime.");
    let mut rl = Editor::<()>::new().expect("Failed to create a new editor.");

    let mut mock_hrt = MockHost::default();

    let realm_clone = rt.realm().clone();

    realm_clone.register_api(jstz_api::ConsoleApi, rt.context());

    realm_clone.register_api(
        jstz_api::KvApi {
            contract_address: contract_address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(jstz_api::TextEncoderApi, rt.context());
    realm_clone.register_api(jstz_api::url::UrlApi, rt.context());
    realm_clone.register_api(jstz_api::http::HttpApi, rt.context());
    realm_clone.register_api(
        jstz_proto::api::LedgerApi {
            contract_address: contract_address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(
        jstz_proto::api::ContractApi {
            contract_address: contract_address.clone(),
        },
        rt.context(),
    );

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let input = line.trim();

                // Check for the exit command.
                if input == "exit" {
                    break Ok(());
                }

                // Add the line to history so you can use arrow keys to recall it
                rl.add_history_entry(line.as_str());

                evaluate(input, &mut rt, &mut mock_hrt);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break Ok(());
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break Ok(());
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break Ok(());
            }
        }
    }
}

fn evaluate(input: &str, rt: &mut Runtime, hrt: &mut (impl HostRuntime + 'static)) {
    let rt_output =
        runtime::with_host_runtime(hrt, || rt.eval(Source::from_bytes(input)));

    match rt_output {
        Ok(res) => {
            if !res.is_undefined() {
                println!(
                    "{}",
                    res.to_string(&mut rt.context())
                        .unwrap()
                        .to_std_string_escaped()
                );
            }
        }
        Err(e) => {
            eprintln!("Uncaught {e}")
        }
    };
}
