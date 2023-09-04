extern crate clap;
use clap::{Arg, App, SubCommand};
use std::str::FromStr;
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::Write;
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub external: String,
}

fn main() {
    // Define the CLI
    let matches = App::new("MyApp")
        .subcommand(
            SubCommand::with_name("run-commands")
                .about("Run multiple commands")
                .after_help("Example usage: ./myapp run-commands \"deploy-bridge --address tz1...; run-contract --self-address tz1... --contract contract_code; deposit --from tz1... --to tz4... --amount 1000; save-session\"")
                .arg(
                    Arg::with_name("COMMANDS")
                        .help("The commands to run, separated by ';' (e.g., 'deploy-bridge --address tz1...; run-contract --self-address tz1... --contract contract_code')")
                        .required(true)
                        .index(1),
                ),
        )
        .get_matches();

    // Match the subcommands
    if let Some(matches) = matches.subcommand_matches("run-commands") {
        let commands = matches.value_of("COMMANDS").unwrap();
        run_commands(commands);
    }
}

fn run_commands(commands: &str) {
    let mut messages = Vec::new();
    for command in commands.split(";") {
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if !parts.is_empty() {
            let subcommand = parts[0];
            let args = &parts[1..];

            println!("** {} **",command);

            match subcommand {
                "deploy-bridge" => {
                    let address = args.iter().position(|&x| x == "--address");
                    if let Some(pos) = address {
                        let address = args[pos + 1];
                        println!("Deploying bridge with address: {}", address);
                        // TODO: Run deploy-bridge function
                        messages.push(Message {
                            external: "fake external message for deploy-bridge".to_string(),
                        });
                    } else {
                        println!("Address is required for deploy-bridge command");
                    }
                }
                "run-contract" => {
                    let self_address = args.iter().position(|&x| x == "--self-address");
                    let contract = args.iter().position(|&x| x == "--contract");
                    if let Some(pos_self_address) = self_address {
                        let self_address = args[pos_self_address + 1];
                        println!("Running contract with self address: {}", self_address);
                        if let Some(pos_contract) = contract {
                            let contract = args[pos_contract + 1];
                            println!("Contract code: {}", contract);
                            // TODO: Run run-contract function
                            messages.push(Message {
                                external: "fake external message for run-contract".to_string(),
                            });
                        } else {
                            println!("Contract code is required for run-contract command");
                        }
                    } else {
                        println!("Self address is required for run-contract command");
                    }
                }
                "deposit" => {
                    let from = args.iter().position(|&x| x == "--from");
                    let to = args.iter().position(|&x| x == "--to");
                    let amount = args.iter().position(|&x| x == "--amount");
                    if let Some(pos_from) = from {
                        let from = args[pos_from + 1];
                        println!("Depositing from address: {}", from);
                        if let Some(pos_to) = to {
                            let to = args[pos_to + 1];
                            println!("To address: {}", to);
                            if let Some(pos_amount) = amount {
                                let amount = args[pos_amount + 1];
                                println!("Amount: {}", amount);
                                // TODO: Run deposit function
                                messages.push(Message {
                                    external: "fake external message for deposit".to_string(),
                                });
                            } else {
                                println!("Amount is required for deposit command");
                            }
                        } else {
                            println!("To address is required for deposit command");
                        }
                    } else {
                        println!("From address is required for deposit command");
                    }
                }
                "save-session" => {
                    save_session(&messages);
                }
                _ => println!("Unknown command: {}", subcommand),
            }

            println!();
        }
    }
}

fn save_session(messages: &Vec<Message>) {
    let filename = "session.json";
    let json = serde_json::to_string(messages).expect("Failed to serialize messages");
    let mut file = File::create(filename).expect("Unable to create file");
    file.write_all(json.as_bytes()).expect("Unable to write data");
    println!("Session saved to {}", filename);
}

