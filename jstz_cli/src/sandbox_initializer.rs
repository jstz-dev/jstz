use std::path::PathBuf;
use std::process::Command;
use std::fs;
use std::time::Duration;
use std::process::Child;
use std::thread::sleep;
use std::fs::File;
use std::path::Path;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author = "Your Name", version = "1.0")]
struct SandboxCli {
    #[command(subcommand)]
    command: SandboxCommands,
}

#[derive(Subcommand)]
enum SandboxCommands {
    /// Initializes the sandboxed client.
    #[command(name = "init-client")]
    InitClient {
        /// Path to the client executable.
        #[arg(short, long)]
        client: String,
        /// Directory containing the script.
        #[arg(short, long)]
        script_dir: PathBuf,
        /// Directory for the node.
        #[arg(short, long)]
        node_dir: PathBuf,
    },
    /// Starts the rollup node.
    #[command(name = "start-rollup")]
    StartRollup {
        /// Path to the client executable.
        #[arg(short, long)]
        client: String,
        /// Kernel for the rollup node.
        #[arg(short, long)]
        kernel: String,
        /// Preimages for the rollup node.
        #[arg(short, long)]
        preimages: String,
        /// Rollup node executable path.
        #[arg(long)]
        rollup_node: String,
        /// Directory for the rollup node.
        #[arg(long)]
        rollup_node_dir: PathBuf,
        /// Directory for logs.
        #[arg(short, long)]
        log_dir: PathBuf,
    },
    /// Starts the sandboxed node.
    #[command(name = "start-node")]
    StartNode {
        /// Path to the node executable.
        #[arg(long)]
        node: String,
        /// Directory for the node.
        #[arg(long)]
        node_dir: PathBuf,
        /// Port for the node.
        #[arg(short, long)]
        port: u16,
        /// RPC port for the node.
        #[arg(short, long)]
        rpc: u16,
        /// Directory containing the script.
        #[arg(short, long)]
        script_dir: PathBuf,
    },
}

fn main() {
    let cli = SandboxCli::parse();

    match cli.command {
        SandboxCommands::InitClient { client, script_dir, node_dir } => {
            init_sandboxed_client(&client, &PathBuf::from(&script_dir), &PathBuf::from(&node_dir));
        },
        SandboxCommands::StartRollup { client, kernel, preimages, rollup_node, rollup_node_dir, log_dir } => {
            start_rollup_node(&client, &kernel, &preimages, &rollup_node, &PathBuf::from(&rollup_node_dir), &PathBuf::from(&log_dir));
        },
        SandboxCommands::StartNode { node, node_dir, port, rpc, script_dir } => {
            match start_sandboxed_node(&node, &PathBuf::from(&node_dir), port, rpc, &PathBuf::from(&script_dir)) {
                Ok(_) => println!("Sandboxed node started successfully."),
                Err(e) => eprintln!("Error starting sandboxed node: {}", e),
            }
        },
    }
}

fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn start_sandboxed_node(node: &str, node_dir: &PathBuf, port: u16, rpc: u16, script_dir: &PathBuf) -> Result<Child, String> {
    // Initialize node config
    run_command(node, &[
        "config", "init",
        "--network", "sandbox",
        "--data-dir", &node_dir.to_str().unwrap(),
        "--net-addr", &format!("127.0.0.1:{}", port),
        "--rpc-addr", &format!("127.0.0.1:{}", rpc),
        "--connections", "0"
    ])?;

    // Generate an identity of the node we want to run
    run_command(node, &[
        "identity", "generate",
        "--data-dir", &node_dir.to_str().unwrap()
    ])?;

    // Start newly configured node in the background
    let child = Command::new(node)
        .args(&[
            "run",
            "--synchronisation-threshold", "0",
            "--network", "sandbox",
            "--data-dir", &node_dir.to_str().unwrap(),
            "--sandbox", &format!("{}/sandbox.json", script_dir.to_str().unwrap())
        ])
        .spawn()
        .expect("Failed to start node");

    Ok(child)
}

fn run_command_silently(command: &str, args: &[&str]) -> bool {
    let output = Command::new(command)
        .args(args)
        .output()
        .expect("Failed to execute command");

    output.status.success()
}

fn wait_for_node_to_initialize(client: &str) {
    if run_command_silently(client, &["rpc", "get", "/chains/main/blocks/head/hash"]) {
        return;
    }

    print!("Waiting for node to initialize");
    while !run_command_silently(client, &["rpc", "get", "/chains/main/blocks/head/hash"]) {
        print!(".");
        sleep(Duration::from_secs(1));
    }

    println!(" done.");
}

fn wait_for_node_to_activate(node_dir: &Path) {
    print!("Waiting for node to activate");
    while !node_dir.join("activated").exists() {
        print!(".");
        sleep(Duration::from_secs(1));
    }

    println!(" done.");
}

fn init_sandboxed_client(client: &str, script_dir: &PathBuf, node_dir: &PathBuf) {
    wait_for_node_to_initialize(client);

    run_command(client, &["bootstrapped"]).expect("Failed to bootstrap client");

    // Add bootstrapped identities
    run_command(client, &["import", "secret", "key", "activator", "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"]).expect("Failed to import activator key");

    // Activate alpha
    run_command(client, &[
        "-block", "genesis",
        "activate", "protocol", "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        "with", "fitness", "1",
        "and", "key", "activator",
        "and", "parameters", &format!("{}/sandbox-params.json", script_dir.to_str().unwrap())
    ]).expect("Failed to activate alpha");

    // Add more bootstrapped accounts
    let keys = [
        "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
        "edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
        "edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
        "edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm"
    ];

    for (i, key) in keys.iter().enumerate() {
        let account_name = format!("bootstrap{}", i + 1);
        run_command(client, &["import", "secret", "key", &account_name, &format!("unencrypted:{}", key)]).expect(&format!("Failed to import {} key", account_name));
    }

    // Create file to communicate to `wait_for_node_activation` process that the node is activated
    File::create(node_dir.join("activated")).expect("Failed to create activated file");

    // Continuously bake
    loop {
        if !run_command_silently(client, &["bake", "for", "--minimal-timestamp"]) {
            break;
        }
        sleep(Duration::from_secs(1));
    }
}

fn originate_rollup(client: &str, kernel: &str, rollup_node_dir: &PathBuf, preimages: &PathBuf) {
    wait_for_node_to_activate(rollup_node_dir);

    run_command(client, &[
        "originate", "smart", "rollup", "jstz_rollup",
        "from", "bootstrap1",
        "of", "kind", "wasm_2_0_0",
        "of", "type", "(pair bytes (ticket unit))",
        "with", "kernel", &format!("file:{}", kernel),
        "--burn-cap", "999"
    ]).expect("Failed to originate rollup");

    // Copy kernel installer preimages to rollup node directory
    let dest_dir = rollup_node_dir.join("wasm_2_0_0");
    fs::create_dir_all(&dest_dir).expect("Failed to create directory");
    fs::copy(preimages, dest_dir).expect("Failed to copy preimages");
}

fn start_rollup_node(client: &str, kernel: &str, preimages: &str, rollup_node: &str, rollup_node_dir: &PathBuf, log_dir: &PathBuf) {
    originate_rollup(client, kernel, rollup_node_dir, &PathBuf::from(preimages));

    let child = Command::new(rollup_node)
        .args(&[
            "run", "operator", "for", "jstz_rollup",
            "with", "operators", "bootstrap2",
            "--data-dir", rollup_node_dir.to_str().unwrap(),
            "--log-kernel-debug",
            "--log-kernel-debug-file", &format!("{}/kernel.log", log_dir.to_str().unwrap())
        ])
        .spawn()
        .expect("Failed to start rollup node");
}