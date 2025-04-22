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
use utils::{AddressOrAlias, Tez};

#[derive(Debug, Parser)]
#[command(name = "jstz", author = "TriliTech <contact@trili.tech>", version)]
enum Command {
    /// 🚀 Deploys a smart function to jstz
    Deploy {
        /// Function code.
        #[arg(value_name = "CODE|PATH", default_value = None, value_hint = clap::ValueHint::FilePath)]
        code: Option<String>,
        /// Initial balance of the function in XTZ.
        #[arg(short, long, default_value = None)]
        balance: Option<Tez>,
        /// Name (or alias) of the function.
        #[arg(long, default_value = None)]
        name: Option<String>,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 🏃 Send a request to a transfer XTZ
    Transfer {
        /// The amount in XTZ to transfer.
        #[arg(value_name = "AMOUNT")]
        amount: Tez,

        /// Destination address or alias of the account (user or smart function).
        #[arg(value_name = "ADDRESS|ALIAS")]
        to: AddressOrAlias,

        /// The maximum amount of gas to be used
        #[arg(short, long, default_value_t = DEFAULT_GAS_LIMIT)]
        gas_limit: u32,

        /// Include response headers in the output
        #[arg(name = "include", short, long, default_value_t = false)]
        include_response_headers: bool,

        /// Specifies the network from the config file, defaulting to the configured default network.
        ///  Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 🏃 Send a request to a deployed smart function
    Run {
        /// The URL containing the functions's address or alias.
        #[arg(value_name = "URL")]
        url: String,
        /// The maximum amount of gas to be used
        #[arg(short, long, default_value_t = DEFAULT_GAS_LIMIT)]
        gas_limit: u32,
        /// The HTTP method used in the request.
        #[arg(name = "method", short, long, default_value = "GET")]
        http_method: String,
        /// The JSON data in the request body.
        #[arg(name = "data", short, long, default_value = None, value_hint = clap::ValueHint::FilePath)]
        json_data: Option<String>,
        /// The amount in XTZ to transfer.
        #[arg(short, long, default_value = None)]
        amount: Option<Tez>,
        /// Specifies the network from the config file, defaulting to the configured default network.
        ///  Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
        /// Flag for logging.
        #[arg(short, long)]
        trace: bool,
        /// Include response headers in the output
        #[arg(name = "include", short, long)]
        include_response_headers: bool,
    },
    /// 🌉 Move XTZ between L1 and jstz with the jstz bridge {n}
    #[command(subcommand)]
    Bridge(bridge::Command),

    /// 🏝️  Start/Stop/Restart the local jstz sandbox
    Sandbox {
        /// Start/Stop/Restart the sandbox in a docker container
        #[clap(long)]
        container: bool,
        #[command(subcommand)]
        command: sandbox::Command,
    },
    /// ⚡️ Start a REPL session with jstz's JavaScript runtime {n}
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(value_name = "ADDRESS|ALIAS", short, long)]
        account: Option<AddressOrAlias>,
    },

    /// 🪵  Explore logs from deployed smart functions
    #[command(subcommand)]
    Logs(logs::Command),
    /// 🔑 Interact with jstz's key-value store {n}
    #[command(subcommand)]
    Kv(kv::Command),

    /// 🧑 Manage jstz accounts
    #[command(subcommand)]
    Account(account::Command),
    /// 🔓 Login to a jstz account
    Login {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// 🚪 Logout from the current jstz account
    Logout {},
    /// 🤔 Display your account info {n}
    #[command(name = "whoami")]
    WhoAmI {},

    /// 📚 Open jstz's docs in your browser
    Docs,
    /// 🐚 Generates shell completions {n}
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
        Command::Sandbox { container, command } => {
            sandbox::exec(container, command).await
        }
        Command::Bridge(bridge_command) => bridge::exec(bridge_command).await,
        Command::Account(account_command) => account::exec(account_command).await,
        Command::Deploy {
            code,
            balance,
            name,
            network,
        } => deploy::exec(code, balance, name, network).await,
        Command::Transfer {
            amount,
            to,
            gas_limit,
            include_response_headers,
            network,
        } => {
            run::exec_transfer(amount, to, gas_limit, include_response_headers, network)
                .await
        }
        Command::Run {
            url,
            http_method,
            gas_limit,
            json_data,
            amount,
            network,
            trace,
            include_response_headers,
        } => {
            let args = run::RunArgs::new(url, http_method, gas_limit);
            run::exec(
                args.set_json_data(json_data)
                    .set_network(network)
                    .set_trace(trace)
                    .set_amount(amount)
                    .set_include_response_headers(include_response_headers),
            )
            .await
        }
        Command::Repl { account } => repl::exec(account).await,
        Command::Logs(logs) => logs::exec(logs).await,
        Command::Login { alias } => account::login(alias).await,
        Command::Logout {} => account::logout().await,
        Command::WhoAmI {} => account::whoami().await,
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
