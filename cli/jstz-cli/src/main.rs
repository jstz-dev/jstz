extern crate clap;
use clap::{Arg, App, SubCommand};
use std::str::FromStr;
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::Write;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use std::io;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub external: String,
}

fn main() {
    let matches = App::new("MyApp")
        .arg(Arg::with_name("script")
            .short('s')
            .long("script")
            .value_name("FILE")
            .help("Specifies a script file to read commands from"))
        .arg(Arg::with_name("self-address")
            .short('a')
            .long("self-address")
            .value_name("ADDRESS")
            .help("Specifies an address to use as a default for the run command"))
        .arg(Arg::with_name("addresses")
            .short('d')
            .long("addresses")
            .value_name("FILE")
            .help("Specifies a JSON file to read addresses from with human-readable names"))
        .arg(Arg::with_name("out")
            .short('o')
            .long("out")
            .value_name("FILE")
            .help("Specifies an output file to create the messages in JSON format when the program exits"))
        .get_matches();

    let mut self_address = matches.value_of("self-address").map(|s| s.to_string());;
    let addresses_file = matches.value_of("addresses");
    let mut out_file = matches.value_of("out").map(|s| s.to_string());;
    let script_file = matches.value_of("script");

    // Load addresses from file
    let mut addresses = HashMap::new();
    if let Some(file) = addresses_file {
        addresses = load_addresses(file);
    }

    let mut messages = Vec::new();
    if let Some(file) = script_file {
        // Read commands from file and run them
        let commands = fs::read_to_string(file).expect("Unable to read file");
        run_command(&commands, &mut addresses, &mut self_address, &mut out_file, &mut messages);
    } else {
        // Enter interactive mode
        let mut input = String::new();
        loop {
            input.clear();
            io::stdin().read_line(&mut input).expect("Failed to read line");
            let command = input.trim();
            if command == "exit" {
                break;
            }
            run_command(command, &mut addresses, &mut self_address, &mut out_file, &mut messages);
        }
    }
}

fn run_command(command: &str, addresses: &mut HashMap<String, String>, self_address: &mut Option<String>, out_file: &mut Option<String>, messages: &mut Vec<Message>) {
    for cmd in command.split(";") {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if !parts.is_empty() {
            let subcommand = parts[0];
            let args = &parts[1..];

            println!("** {} **", cmd);

            match subcommand {
                "run" => {
                    if args.len() < 2 {
                        println!("Usage: run contract <addr> <code>");
                        return;
                    }
                    let addr = args[1];
                    let code = args[2];
                    run_contract(addr, code, messages);
                }
                "deploy" => {
                    if args.len() < 2 {
                        println!("Usage: deploy bridge <addr>");
                        return;
                    }
                    let addr = args[1];
                    deploy_bridge(addr, messages);
                }
                "deposit" => {
                    if args.len() < 6 {
                        println!("Usage: deposit <amount> tez from <addr1> to <addr2>");
                        return;
                    }
                    let amount = args[0];
                    let addr1 = args[3];
                    let addr2 = args[5];
                    deposit(amount, addr1, addr2, messages);
                }
                "load" => {
                    if args.len() < 2 {
                        println!("Usage: load addresses <file>");
                        return;
                    }
                    let file = args[1];
                    *addresses = load_addresses(file);
                }
                "set" => {
                    if args.len() < 3 {
                        println!("Usage: set self address <addr>");
                        return;
                    }
                    let addr = args[2];
                    *self_address = Some(addr.to_string());
                }
                "write" => {
                    if args.len() > 2 {
                        println!("Usage: write inputs <filename>");
                        return;
                    }
                    let file = if args.len() == 2 { args[1] } else { out_file.as_deref().unwrap() };
                    save_session(&messages, file);
                }
                _ => {
                    println!("Unknown command: {}", subcommand);
                }
            }
        }
    }
}

fn load_addresses(file: &str) -> HashMap<String, String> {
    let contents = fs::read_to_string(file).expect("Unable to read file");
    let addresses: HashMap<String, String> = serde_json::from_str(&contents).expect("Unable to parse JSON");
    addresses
}

fn run_contract(addr: &str, code: &str, messages: &mut Vec<Message>) {
    // Check if the code argument is a file or a block of code
    if Path::new(code).exists() {
        let code = fs::read_to_string(code).expect("Unable to read file");
        messages.push(Message { external: format!("run-contract: {}, {}", addr, &code) });
    } else {
        messages.push(Message { external: format!("run-contract: {}, {}", addr, &code) });
    }
}

fn deploy_bridge(addr: &str, messages: &mut Vec<Message>) {
    messages.push(Message { external: format!("deploy-bridge: {}", addr) });
}

fn deposit(amount: &str, addr1: &str, addr2: &str, messages: &mut Vec<Message>) {
    messages.push(Message { external: format!("deposit: {} tez from {} to {}", amount, addr1, addr2) });
}

fn save_session(messages: &Vec<Message>, out_file: &str) {
    let json = serde_json::to_string(messages).expect("Unable to convert to JSON");
    fs::write(out_file, json).expect("Unable to write file");
}