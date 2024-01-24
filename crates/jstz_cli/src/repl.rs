use crate::js_logger::PrettyLogger;
use anyhow::Result;
use boa_engine::{js_string, JsResult, JsValue, Source};
use jstz_api::{
    encoding::EncodingApi, http::HttpApi, js_log::set_js_logger, stream::StreamApi,
    url::UrlApi, urlpattern::UrlPatternApi, ConsoleApi, KvApi,
};
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::Kv,
    runtime::{self, Runtime},
};
use jstz_proto::api::{ContractApi, LedgerApi};
use rustyline::{
    completion::Completer, error::ReadlineError, highlight::Highlighter, hint::Hinter,
    validate::Validator, Editor, Helper,
};
use std::borrow::Cow;
use tezos_smart_rollup_mock::MockHost;

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use crate::{config::Config, debug_api::DebugApi};

struct JsHighlighter {
    ss: SyntaxSet,
    syntax: SyntaxReference,
    theme: Theme,
}

impl JsHighlighter {
    pub fn new() -> Self {
        // Initialize syntax set and theme set only once
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        // Find JavaScript syntax and a theme
        let syntax = ss.find_syntax_by_extension("js").unwrap();
        let theme = &ts.themes["base16-ocean.dark"];

        JsHighlighter {
            syntax: syntax.clone(),
            theme: theme.clone(),
            ss,
        }
    }

    fn apply_foreground_only(&self, styles: &[(Style, &str)]) -> String {
        styles
            .iter()
            .map(|&(style, text)| {
                let color = style.foreground;
                format!(
                    "\x1b[38;2;{};{};{}m{}\x1b[0m",
                    color.r, color.g, color.b, text
                )
            })
            .collect()
    }

    fn highlight(&self, input: &str) -> String {
        let mut h = HighlightLines::new(&self.syntax, &self.theme);
        let ranges: Vec<(Style, &str)> = h.highlight(input, &self.ss);
        self.apply_foreground_only(&ranges)
    }
}

impl Highlighter for JsHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Owned(self.highlight(line))
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        true
    }
}

impl Helper for JsHighlighter {}

impl Hinter for JsHighlighter {
    type Hint = String;
}

impl Validator for JsHighlighter {}

impl Completer for JsHighlighter {
    type Candidate = String;
}

pub fn exec(self_address: Option<String>, cfg: &Config) -> Result<()> {
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

    let mut rl = Editor::<JsHighlighter, _>::new()?;
    rl.set_helper(Some(JsHighlighter::new()));

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
