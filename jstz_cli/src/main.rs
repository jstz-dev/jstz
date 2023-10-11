use anyhow::Result;
use clap::Parser;

mod bridge;
mod config;
mod deploy;
mod octez;
mod repl;
mod run;
mod sandbox;

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
    /// Deploys a smart function
    Deploy {
        /// Address used when deploying the contract
        #[arg(short, long)]
        self_address: String,
        /// Function code.
        #[arg(short, long)]
        function_code: String,
        /// Initial balance
        #[arg(short, long)]
        balance: u64,
    },
    /// Run a smart function using a specified URL.
    Run {
        /// The URL containing the functions's address or alias.
        #[arg(value_name = "URL")]
        url: String,
        /// The address of the caller (or referrer)
        #[arg(value_name = "referrer")]
        referrer: String,
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
}

fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Sandbox(sandbox_command) => sandbox::exec(cfg, sandbox_command),
        Command::Bridge(bridge_command) => bridge::exec(bridge_command, cfg),
        Command::Deploy {
            self_address,
            function_code,
            balance,
        } => deploy::exec(self_address, function_code, balance, cfg),
        Command::Run {
            url,
            referrer,
            http_method,
            json_data,
        } => run::exec(cfg, referrer, url, http_method, json_data),
        Command::Repl { self_address } => repl::exec(self_address),
    }
}

fn main() -> Result<()> {
    let command = Command::parse();

    let mut cfg = Config::load()?;

    exec(command, &mut cfg)
}
