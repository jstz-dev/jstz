use crate::js_logger::PrettyLogger;
use anyhow::Result;
use boa_engine::{js_string, JsResult, JsValue, Source};
use jstz_api::{
    encoding::EncodingApi, http::HttpApi, js_log::set_js_logger, stream::StreamApi,
    url::UrlApi, urlpattern::UrlPatternApi, ConsoleApi, KvApi,
};
use jstz_core::host::HostRuntime;
use jstz_core::{
    host_defined,
    kv::Kv,
    runtime::{self, Runtime},
};
use jstz_proto::api::{ContractApi, LedgerApi};
use rustyline::{error::ReadlineError, Editor};
use tezos_smart_rollup_mock::MockHost;

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

use crossterm::{
    cursor::MoveUp,
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::io::{stdout, Write};

use crate::{config::Config, debug_api::DebugApi};

pub fn exec(self_address: Option<String>, cfg: &Config) -> Result<()> {
    // Initialize syntax set and theme set
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    // Find JavaScript syntax and a theme (e.g., base16-ocean.dark)
    let syntax = ss.find_syntax_by_extension("js").unwrap();
    let theme = &ts.themes["base16-ocean.dark"];

    let account = cfg.accounts.account_or_current(self_address)?;
    let address = account.address();

    let mut rt = Runtime::new(usize::MAX).expect("Failed to create a new runtime.");

    {
        let context = rt.context();
        host_defined!(context, mut host_defined);

        let kv = Kv::new();
        let tx = kv.begin_transaction();

        host_defined.insert(kv);
        host_defined.insert(tx);
    }
    set_js_logger(&PrettyLogger);

    let mut rl = Editor::<(), _>::new().expect("Failed to create a new editor.");

    let mut mock_hrt = MockHost::default();

    let realm_clone = rt.realm().clone();

    realm_clone.register_api(ConsoleApi, rt.context());

    realm_clone.register_api(
        KvApi {
            contract_address: address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(EncodingApi, rt.context());
    realm_clone.register_api(StreamApi, rt.context());
    realm_clone.register_api(UrlApi, rt.context());
    realm_clone.register_api(UrlPatternApi, rt.context());
    realm_clone.register_api(HttpApi, rt.context());
    realm_clone.register_api(
        LedgerApi {
            contract_address: address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(
        ContractApi {
            contract_address: address.clone(),
        },
        rt.context(),
    );
    realm_clone.register_api(DebugApi, rt.context());

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

                // Syntax highlighting
                stdout().execute(Clear(ClearType::CurrentLine)).unwrap();
                stdout().execute(MoveUp(1)).unwrap();
                stdout().execute(Clear(ClearType::CurrentLine)).unwrap();

                print!(">> ");
                highlight_and_print(input, &ss, syntax, theme);
                stdout().flush().unwrap();

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

fn apply_foreground_only(styles: &[(Style, &str)]) -> String {
    let mut result = String::new();

    for &(style, text) in styles {
        let color = style.foreground;
        let color_string = format!("\x1b[38;2;{};{};{}m", color.r, color.g, color.b);

        result.push_str(&color_string);
        result.push_str(text);

        result.push_str("\x1b[0m");
    }

    result
}

fn highlight_and_print(
    input: &str,
    ss: &SyntaxSet,
    syntax: &SyntaxReference,
    theme: &Theme,
) {
    let mut h = HighlightLines::new(syntax, theme);
    for line in LinesWithEndings::from(input) {
        let ranges: Vec<(Style, &str)> = h.highlight(line, ss);
        let formatted_line = apply_foreground_only(&ranges);
        println!("{}", formatted_line);
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
