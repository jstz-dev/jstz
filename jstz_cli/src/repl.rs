use anyhow::Result;
use boa_engine::{js_string, JsResult, JsValue, Source};
use jstz_api::{http::HttpApi, url::UrlApi, ConsoleApi, KvApi, TextEncoderApi};
use jstz_core::host::HostRuntime;
use jstz_core::{
    host_defined,
    kv::Kv,
    runtime::{self, Runtime},
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::api::{ContractApi, LedgerApi};
use rustyline::{error::ReadlineError, Editor};
use tezos_smart_rollup_mock::MockHost;

pub fn exec(self_address: Option<String>) -> Result<()> {
    let mock_address_string = "tz4RepLRepLRepLRepLRepLRepLRepN7Cu8j";

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

    {
        let context = rt.context();
        host_defined!(context, mut host_defined);

        let kv = Kv::new();
        let tx = kv.begin_transaction();

        host_defined.insert(kv);
        host_defined.insert(tx);
    }

    let mut rl = Editor::<(), _>::new().expect("Failed to create a new editor.");

    let mut mock_hrt = MockHost::default();

    let realm_clone = rt.realm().clone();

    realm_clone.register_api(ConsoleApi, rt.context());

    realm_clone.register_api(
        KvApi {
            contract_address: contract_address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(TextEncoderApi, rt.context());
    realm_clone.register_api(UrlApi, rt.context());
    realm_clone.register_api(HttpApi, rt.context());
    realm_clone.register_api(
        LedgerApi {
            contract_address: contract_address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(
        ContractApi {
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
                rl.add_history_entry(line.as_str())?;

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
    let rt_output = runtime::with_host_runtime(hrt, || -> JsResult<JsValue> {
        let value = rt.eval(Source::from_bytes(input))?;
        jstz_core::future::block_on(async {
            rt.run_event_loop().await;
            rt.resolve_value(&value).await
        })
    });

    match rt_output {
        Ok(res) => {
            if !res.is_undefined() {
                println!(
                    "{}",
                    if res.is_callable() {
                        res.to_string(rt.context()).unwrap().to_std_string_escaped()
                    } else {
                        res.display().to_string()
                    },
                );
            }
            if let Err(err) =
                rt.global_object()
                    .set(js_string!("_"), res, false, rt.context())
            {
                println!("Couldn't set '_' property: {err}");
            }
        }
        Err(e) => {
            eprintln!("Uncaught {e}")
        }
    }
}
