use clap::Parser;
use clap_complete::Shell;

mod account;
mod bridge;
mod completions;
mod config;
mod deploy;
mod docs;
mod error;
mod jstz;
mod kv;
mod logs;
mod repl;
mod run;
mod sandbox;
mod term;
mod utils;

use config::{Config, NetworkName};
use error::Result;
use log::debug;
use run::DEFAULT_GAS_LIMIT;
use utils::AddressOrAlias;

#[derive(Debug, Parser)]
#[command(name = "jstz", author = "TriliTech <contact@trili.tech>", version)]
enum Command {
    /// 🚀 Deploys a smart function to jstz.
    Deploy {
        /// Function code.
        #[arg(value_name = "CODE|PATH", default_value = None)]
        code: Option<String>,
        /// Initial balance of the function.
        #[arg(short, long, default_value_t = 0)]
        balance: u64,
        /// Name (or alias) of the function.
        #[arg(long, default_value = None)]
        name: Option<String>,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
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
        /// Specifies the network from the config file, defaulting to the configured default network.
        ///  Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 🌉 Move CTEZ between L1 and jstz with the jstz bridge.
    #[command(subcommand)]
    Bridge(bridge::Command),

    /// 🏝️  Start/Stop/Restart the local jstz sandbox.
    #[command(subcommand)]
    Sandbox(sandbox::Command),
    /// ⚡️ Start a REPL session with jstz's JavaScript runtime.
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(value_name = "ADDRESS|ALIAS", short, long)]
        account: Option<AddressOrAlias>,
    },

    /// 🪵  Explore logs from deployed smart functions.
    #[command(subcommand)]
    Logs(logs::Command),
    /// 🔑 Interact with jstz's key-value store.
    #[command(subcommand)]
    Kv(kv::Command),

    /// 🧑 Manage jstz accounts.
    #[command(subcommand)]
    Account(account::Command),
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

    /// 📚 Open jstz's docs in your browser.
    Docs,
    /// 🐚 Generates shell completions.
    Completions {
        /// The shell to generate completions for
        #[arg(long, short)]
        shell: Shell,
    },
}

async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Docs => docs::exec(),
        Command::Completions { shell } => completions::exec(shell),
        Command::Sandbox(sandbox_command) => sandbox::exec(sandbox_command).await,
        Command::Bridge(bridge_command) => bridge::exec(bridge_command),
        Command::Account(account_command) => account::exec(account_command).await,
        Command::Deploy {
            code,
            balance,
            name,
            network,
        } => deploy::exec(code, balance, name, network).await,
        Command::Run {
            url,
            http_method,
            gas_limit,
            json_data,
            network,
        } => run::exec(url, http_method, gas_limit, json_data, network).await,
        Command::Repl { account } => repl::exec(account),
        Command::Logs(logs) => logs::exec(logs).await,
        Command::Login { alias } => account::login(alias),
        Command::Logout {} => account::logout(),
        Command::WhoAmI {} => account::whoami(),
        Command::Kv(kv_command) => kv::exec(kv_command).await,
    }
}

fn print_custom_help() {
    let custom_help_message = "\
Usage: jstz <COMMAND>

Commands:
  deploy       🚀 Deploys a smart function to jstz
  run          🏃 Send a request to a deployed smart function
  bridge       🌉 Move CTEZ between L1 and jstz with the jstz bridge

  sandbox      🏝️  Start/Stop/Restart the local jstz sandbox
  repl         ⚡️ Start a REPL session with jstz's JavaScript runtime

  logs         🪵  Explore logs from deployed smart functions
  kv           🔑 Interact with jstz's key-value store

  account      🧑 Manage jstz accounts
  login        🔓 Login to a jstz account
  logout       🚪 Logout from the current jstz account
  whoami       🤔 Display your account info
  
  docs         📚 Open jstz's docs in your browser
  completions  🐚 Generates shell completions
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version";
    println!("{}", custom_help_message);
}

#[tokio::main]
async fn main() {
    term::init_logger();

    let args: Vec<String> = std::env::args().collect();
    if (args.contains(&"--help".to_string())
        || args.contains(&"-h".to_string())
        || args.contains(&"help".to_string()))
        && args.len() == 2
    {
        print_custom_help();
        return;
    }

    let command = Command::parse();

    debug!("Command: {:?}", command);

    if let Err(err) = exec(command).await {
        error::print(&err);
        std::process::exit(1);
    }
}
