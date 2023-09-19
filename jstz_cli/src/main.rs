use clap::{Parser, Subcommand};

mod deposit;
mod deploy;
mod run_contract;
mod utils;
//mod sandbox;
//mod repl;
mod config; 

use crate::deposit::deposit;
use crate::deploy::deploy;
use crate::run_contract::run_contract;
//use crate::sandbox::sandbox_start;
//use crate::sandbox::sandbox_stop;
//use crate::sandbox::repl;
use config::Config;


#[derive(Parser)]
#[command(author, version)]
struct JstzCli {
    /// Path to the jstz configuration file.
    #[arg(short, long, value_name = "CONFIG_FILE", default_value = "jstz.json")]
    config_file: String,

    #[command(subcommand)]
    command: JstzCommand,
}

#[derive(Subcommand)]
enum JstzCommand {
    /// Commands related to the jstz sandbox.
    #[command(subcommand)]
    Sandbox(SandboxCommand),
    /// Deposits from an existing L1 sandbox address to a L2 sandbox address.
    #[command(name = "bridge-deposit")]
    BridgeDeposit {
        /// The L1 sandbox address or alias to withdraw from.
        #[arg(short, long)]
        from: String,
        /// The L2 sandbox address or alias to deposit to.
        #[arg(short, long)]
        to: String,
        /// The amount in ctez to transfer.
        #[arg(short, long)]
        amount: u64,
    },
    /// Publishes the given script to the local sandbox.
    Deploy {
        /// Path to the contract script.
        #[arg(value_name = "SCRIPT_PATH")]
        script: String,
        /// Alias for the address of the deployed contract.
        #[arg(short, long)]
        name: String,
    },
    /// Run a contract using a specified URL.
    Run {
        /// The URL containing the contract's address or alias.
        #[arg(value_name = "URL")]
        url: String,
        /// The code of the contract.
        #[arg(value_name = "CONTRACT_CODE")]
        contract_code: String,
        /*
        /// The HTTP method used in the request.
        #[arg(name="request", short, long, default_value = "GET")]
        http_method: Option<String>,
        /// The JSON data in the request body.
        #[arg(name="data", short, long, default_value = "{}")]
        json_data: Option<String>,
        */
    },
    /// Start a REPL session.
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(short, long)]
        self_address: Option<String>,
    },
}

#[derive(Subcommand)]
enum SandboxCommand {
    /// Starts a jstz sandbox, starting an octez-node, rollup node, baker, and deploys the jstz rollup kernel and jstz bridge.
    Start,
    /// Stops the currently running jstz sandbox.
    Stop,
}


fn main() {
    let cli = JstzCli::parse();

    let mut cfg = Config::default();
    if let Err(e) = cfg.load_from_file() {
        // Handle the error from loading the configuration
        eprintln!("Failed to load the config file: {}", e);
        return;
    }

    match cli.command {
        JstzCommand::Sandbox(cmd) => match cmd {
            SandboxCommand::Start => {
                println!("Starting the jstz sandbox...");
                //sandbox_start();
            }
            SandboxCommand::Stop => {
                println!("Stopping the jstz sandbox...");
                //sandbox_stop();
            }
        },
        JstzCommand::BridgeDeposit { mut from, mut to, amount } => {
            println!("Depositing {} Tez from {} to {}", amount, from, to);
            if let Some(alias) = cfg.get_tz4_alias(&from) {
                println!("Using alias for {}: {}", from, alias);
                from = alias;
            }
            if let Some(alias) = cfg.get_tz4_alias(&to) {
                println!("Using alias for {}: {}", to, alias);
                to = alias;
            }

            deposit(from, to, amount, &cfg);
        },
        JstzCommand::Deploy { mut script, name } => {
            println!("Deploying script {} with alias {}", script, name);
            if let Some(alias) = cfg.get_name_alias(&name) {
                println!("Using alias for {} instead of script: {}", name, alias);
                script = alias;
            }
            deploy(script, &cfg);
        },
        JstzCommand::Run { mut url, contract_code } => {
            println!("Running {} with code {}", url, contract_code);
            if let Some(alias) = cfg.get_url_alias(&url) {
                println!("Using alias for {}: {}", url, alias);
                url = alias;
            }
            run_contract(url, contract_code, &cfg);
        },
        JstzCommand::Repl { self_address } => {
            if let Some(address) = self_address {
                println!("Starting REPL with self address: {}", address);
                //repl(address)
            } else {
                println!("Starting REPL without a self address");
                //repl()
            }
        },
    }

    cfg.save_to_file();
}
