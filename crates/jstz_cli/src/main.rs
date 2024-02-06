use clap::Parser;

mod account;
mod bridge;
mod config;
mod deploy;
mod error;
mod jstz;
mod kv;
mod logs;
mod repl;
mod run;
mod sandbox;
mod term;
mod utils;

use config::Config;
use error::Result;
use log::debug;
use run::DEFAULT_GAS_LIMIT;
use utils::AddressOrAlias;

#[derive(Debug, Parser)]
#[command(name = "jstz", author = "TriliTech <contact@trili.tech>", version)]
enum Command {
    /// 🏝️ Interact with the jstz sandbox.
    #[command(subcommand)]
    Sandbox(sandbox::Command),
    /// 🌉 Interact with the jstz bridge.
    #[command(subcommand)]
    Bridge(bridge::Command),
    /// 🧑 Interact with jstz's accounts.
    #[command(subcommand)]
    Account(account::Command),
    /// 🔑 Interact with jstz's key-value store.
    #[command(subcommand)]
    Kv(kv::Command),
    /// 🚀 Deploys a smart function to jstz.
    Deploy {
        /// Function code.
        #[arg(value_name = "CODE|PATH", default_value = None)]
        code: Option<String>,
        /// Initial balance of the function.
        #[arg(short, long, default_value_t = 0)]
        balance: u64,
        /// Name (or alias) of the function.
        #[arg(short, long, default_value = None)]
        name: Option<String>,
    },
    /// 🏃 Send a request to a deployed smart function.
    Run {
        /// The URL containing the functions's address or alias.
        #[arg(value_name = "URL")]
        url: String,
        /// The maximum amount of gas to be used
        #[arg(short, long, default_value_t = DEFAULT_GAS_LIMIT)]
        gas_limit: u32,
        /// The HTTP method used in the request.
        #[arg(name = "request", short, long, default_value = "GET")]
        http_method: String,
        /// The JSON data in the request body.
        #[arg(name = "data", short, long, default_value = None)]
        json_data: Option<String>,
    },
    /// ⚡️ Start a REPL session with jstz's JavaScript runtime.
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(value_name = "ADDRESS|ALIAS", short, long)]
        account: Option<AddressOrAlias>,
    },
    /// 🪵 Interact with logs from deployed smart functions.
    #[command(subcommand)]
    Logs(logs::Command),
    /// 🔓 Login to a jstz account.
    Login {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// 🚪 Logout from the current jstz account.
    Logout {},
    /// 🤔 Display your account info.
    #[command(name = "whoami")]
    WhoAmI {},
}

async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Sandbox(sandbox_command) => sandbox::exec(sandbox_command),
        Command::Bridge(bridge_command) => bridge::exec(bridge_command),
        Command::Account(account_command) => account::exec(account_command).await,
        Command::Deploy {
            code,
            balance,
            name,
        } => deploy::exec(code, balance, name).await,
        Command::Run {
            url,
            http_method,
            gas_limit,
            json_data,
        } => run::exec(url, http_method, gas_limit, json_data).await,
        Command::Repl { account } => repl::exec(account),
        Command::Logs(logs) => logs::exec(logs).await,
        Command::Login { alias } => account::login(alias),
        Command::Logout {} => account::logout(),
        Command::WhoAmI {} => account::whoami(),
        Command::Kv(kv_command) => kv::exec(kv_command).await,
    }
}

#[tokio::main]
async fn main() {
    term::init_logger();

    let command = Command::parse();

    debug!("Command: {:?}", command);

    if let Err(err) = exec(command).await {
        error::print(&err);
        std::process::exit(1);
    }
}
