use std::{borrow::Cow, fmt::Write};

use boa_engine::{js_string, Context, JsResult, JsValue, Source};
use jstz_api::{js_log::set_js_logger, stream::StreamApi};
use jstz_core::{
    host::HostRuntime,
    kv::Transaction,
    runtime::{self, Runtime},
};
use jstz_proto::executor::smart_function::{register_jstz_apis, register_web_apis};
use log::{debug, error, info, warn};
use rustyline::{
    completion::Completer, error::ReadlineError, highlight::Highlighter, hint::Hinter,
    validate::Validator, Editor, Helper,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};
use tezos_smart_rollup_mock::MockHost;

use crate::{
    config::Config,
    error::{anyhow, user_error, Result},
    utils::AddressOrAlias,
};

mod debug_api;
mod js_logger;

use debug_api::DebugApi;
use js_logger::PrettyLogger;

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
            .fold(String::new(), |mut output, &(style, text)| {
                let color = style.foreground;
                let _ = write!(
                    &mut output,
                    "\x1b[38;2;{};{};{}m{}\x1b[0m",
                    color.r, color.g, color.b, text
                );
                output
            })
    }

    fn highlight(&self, input: &str) -> String {
        let mut h = HighlightLines::new(&self.syntax, &self.theme);
        let ranges: Vec<(Style, &str)> = h
            .highlight_line(input, &self.ss)
            .expect("Failed to highlight line");
        self.apply_foreground_only(&ranges)
    }
}

impl Highlighter for JsHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Owned(self.highlight(line))
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
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

const DEFAULT_SMART_FUNCTION_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
const DEFAULT_GAS_LIMIT: usize = usize::MAX;
const DEFAULT_RANDOM_SEED: u64 = 42;

pub fn exec(account: Option<AddressOrAlias>) -> Result<()> {
    let cfg = Config::load()?;

    let address = match account {
        Some(account) => account.resolve(&cfg)?.clone(),
        None => DEFAULT_SMART_FUNCTION_ADDRESS
            .parse()
            .expect("`DEFAULT_SMART_FUNCTION_ADDRESS` is an invalid address."),
    };
    debug!("resolved `account` -> {:?}", address);

    // 1. Setup editor
    let mut rl = Editor::<JsHighlighter, _>::new()?;
    rl.set_helper(Some(JsHighlighter::new()));

    // 2. Setup runtime
    let mut rt = Runtime::new(DEFAULT_GAS_LIMIT)
        .map_err(|_| anyhow!("Failed to initialize jstz's JavaScript runtime."))?;
    let mut tx = Transaction::default();
    tx.begin();

    set_js_logger(&PrettyLogger);

    let mut mock_hrt = MockHost::default();
    let realm = rt.realm().clone();

    register_web_apis(&realm, &mut rt);
    register_jstz_apis(&realm, &address, DEFAULT_RANDOM_SEED, &mut rt);

    // realm.register_api(ConsoleApi, rt.context());
    realm.register_api(StreamApi, rt.context());
    realm.register_api(DebugApi, rt.context());

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

                let result = evaluate(input, &mut rt, &mut mock_hrt, &mut tx);
                print_rt_result(result, &mut rt);
            }
            Err(ReadlineError::Interrupted) => {
                info!("CTRL-C");
                break Ok(());
            }
            Err(ReadlineError::Eof) => {
                info!("CTRL-D");
                break Ok(());
            }
            Err(err) => {
                break Err(user_error!("Unexpected REPL error.").context(err));
            }
        }
    }
}

fn print_rt_result(result: JsResult<JsValue>, context: &mut Context) {
    match result {
        Ok(res) => {
            if !res.is_undefined() {
                info!(
                    "{}",
                    if res.is_callable() {
                        res.to_string(context)
                            .expect("Expected [[toString]] to be defined.")
                            .to_std_string_escaped()
                    } else {
                        res.display().to_string()
                    },
                );
            }

            if context
                .global_object()
                .set(js_string!("_"), res, false, context)
                .is_err()
            {
                warn!("Couldn't set '_' to REPL result.");
            }
        }
        Err(e) => {
            error!("Uncaught {e}")
        }
    }
}

fn evaluate(
    input: &str,
    rt: &mut Runtime,
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
) -> JsResult<JsValue> {
    runtime::enter_js_host_context(hrt, tx, || {
        let result = rt.eval(Source::from_bytes(input))?;
        jstz_core::future::block_on(async {
            rt.run_event_loop().await;
            rt.resolve_value(&result).await
        })
    })
}
