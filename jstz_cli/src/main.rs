use anyhow::Result;
use clap::Parser;
use tokio;

mod account;
mod bridge;
mod config;
mod deploy;
mod jstz;
mod logs;
mod octez;
mod repl;
mod run;
mod sandbox;
mod utils;

use config::Config;

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
        /// Address used when deploying the contract
        #[arg(short, long, default_value = None)]
        self_address: Option<String>,
        /// Initial balance
        #[arg(short, long, default_value_t = 0)]
        balance: u64,
        /// Function code.
        #[arg(value_name = "function_code", default_value = None)]
        function_code: Option<String>,
    },
    /// Run a smart function using a specified URL.
    Run {
        /// The URL containing the functions's address or alias.
        #[arg(value_name = "URL")]
        url: String,
        /// The address of the caller (or referrer)
        #[arg(value_name = "referrer", default_value = None)]
        referrer: Option<String>,
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
        self_address: Option<String>,
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
}

async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Sandbox(sandbox_command) => sandbox::exec(cfg, sandbox_command),
        Command::Bridge(bridge_command) => bridge::exec(bridge_command, cfg),
        Command::Account(account_command) => account::exec(account_command, cfg),
        Command::Deploy {
            self_address,
            function_code,
            balance,
        } => deploy::exec(self_address, function_code, balance, cfg).await,
        Command::Run {
            url,
            referrer,
            http_method,
            json_data,
        } => run::exec(cfg, referrer, url, http_method, json_data).await,
        Command::Repl { self_address } => repl::exec(self_address, cfg),
        Command::Logs(logs) => logs::exec(logs, cfg),
        Command::Login { alias } => account::login(alias, cfg),
        Command::Logout {} => account::logout(cfg),
        Command::WhoAmI {} => account::whoami(cfg),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let command = Command::parse();

    let mut cfg = Config::load()?;

    exec(command, &mut cfg).await
}
