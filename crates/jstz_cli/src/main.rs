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
use run::DEFAULT_GAS_LIMIT;
use utils::AddressOrAlias;

#[derive(Parser)]
#[command(author, version)]
enum Command {
    /// Commands related to the jstz sandbox.
    #[command(subcommand)]
    Sandbox(sandbox::Command),
    /// Commands related to the jstz bridge
    #[command(subcommand)]
    Bridge(bridge::Command),
    /// Commands related to the account management
    #[command(subcommand)]
    Account(account::Command),
    /// Deploys a smart function
    Deploy {
        /// Function code.
        #[arg(default_value = None)]
        code: Option<String>,
        /// Initial balance
        #[arg(short, long, default_value_t = 0)]
        balance: u64,
        /// Name
        #[arg(short, long, default_value = None)]
        name: Option<String>,
    },
    /// Run a smart function using a specified URL.
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
    /// Start a REPL session.
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(short, long)]
        account: Option<AddressOrAlias>,
    },
    /// Commands related to the logs.
    #[command(subcommand)]
    Logs(logs::Command),
    /// Logs in to an account
    Login {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// Logs out of the current account
    Logout {},
    /// Shows the current account
    #[command(name = "whoami")]
    WhoAmI {},
    /// Commands realted to the KV store
    #[command(subcommand)]
    Kv(kv::Command),
}

async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Sandbox(sandbox_command) => sandbox::exec(sandbox_command).await,
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
    let command = Command::parse();

    if let Err(err) = exec(command).await {
        error::print(&err);
        std::process::exit(1);
    }
}
