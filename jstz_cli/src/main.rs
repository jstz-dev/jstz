use clap::{Parser, Subcommand};

mod deposit;
mod deploy;
mod run_contract;
//mod sandbox;

use crate::deposit::deposit;
use crate::deploy::deploy;
use crate::run_contract::run_contract;
//use crate::sandbox::sandbox_start;
//use crate::sandbox::sandbox_stop;


#[derive(Parser)]
#[command(author, version)]
struct JstzCli {
    /// Path to the jstz configuration file.
    #[arg(short, long, value_name = "CONFIG_FILE", default_value = "jstz.json")]
    config_file: String,

    #[command(subcommand)]
    command: JstzCommands,
}

#[derive(Subcommand)]
enum JstzCommands {
    /// Commands related to the jstz sandbox.
    #[command(subcommand)]
    Sandbox(SandboxCommands),
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
        /// The HTTP method used in the request.
        #[arg(name="X", short, long, default_value = "GET")]
        http_method: String,
        /// The JSON data in the request body.
        #[arg(name="d", short, long, default_value = "{}")]
        json_data: String,
    },
    /// Start a REPL session.
    Repl {
        /// Sets the address of the REPL environment.
        #[arg(short, long)]
        self_address: Option<String>,
    },
}

#[derive(Subcommand)]
enum SandboxCommands {
    /// Starts a jstz sandbox, starting an octez-node, rollup node, baker, and deploys the jstz rollup kernel and jstz bridge.
    Start,
    /// Stops the currently running jstz sandbox.
    Stop,
}


fn main() {
    let cli = JstzCli::parse();

    match cli.command {
        JstzCommands::Sandbox(cmd) => match cmd {
            SandboxCommands::Start => {
                println!("Starting the jstz sandbox...");
                //sandbox_start();
            }
            SandboxCommands::Stop => {
                println!("Stopping the jstz sandbox...");
                //sandbox_stop();
            }
        },
        JstzCommands::BridgeDeposit { from, to, amount } => {
            println!("Depositing {} Tez from {} to {}", amount, from, to);
            deposit(from, to, amount);
        },
        JstzCommands::Deploy { script, name } => {
            println!("Deploying script {} with alias {}", script, name);
            deploy(script, name);
        },
        JstzCommands::Run { url, http_method, json_data } => {
            println!("Running {} with method {} and data {}", url, http_method, json_data);
            run_contract(url, http_method, json_data);
        },
        JstzCommands::Repl { self_address } => {
            if let Some(address) = self_address {
                println!("Starting REPL with self address: {}", address);
            } else {
                println!("Starting REPL without a self address");
            }
        },
    }
}
