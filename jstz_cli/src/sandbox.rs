use std::process::{Command, Child};
use std::env;
use std::path::PathBuf;
use tempfile::Builder;
use std::fs::File;
use std::fs;
use std::thread::sleep;
use std::time::Duration;
use std::path::Path;
use std::thread;
use serde::{Serialize, Deserialize};
use config::Config;

pub fn sandbox_start(mut cfg: Config) {
    // Check if sandbox is already running
    if cfg.get_is_sandbox_running().unwrap_or(false) {
        println!("Error: Sandbox is already running!");
        return;
    }

    let current_dir = env::current_dir().expect("Failed to get current directory");
    let script_dir = current_dir.parent().expect("Failed to get parent directory");
    let root_dir = script_dir.parent().expect("Failed to get root directory");
    let log_dir = root_dir.join("logs");

    let port = 19730;
    let rpc = 18730;

    // Create temporary directories TODO: Likely should be changed to detect an existing one
    let node_dir = Builder::new().prefix("octez_node").tempdir().expect("Failed to create temp dir for node");
    let rollup_node_dir = Builder::new().prefix("octez_smart_rollup_node").tempdir().expect("Failed to create temp dir for rollup node");
    let client_dir = Builder::new().prefix("octez_client").tempdir().expect("Failed to create temp dir for client");

    // Set environment variable
    //env::set_var("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");

    let client = format!("{}/octez-client -base-dir {} -endpoint http://127.0.0.1:{}", root_dir.to_str().unwrap(), client_dir.path().to_str().unwrap(), rpc);
    let rollup_node = format!("{}/octez-smart-rollup-node -base-dir {} -endpoint http://127.0.0.1:{}", root_dir.to_str().unwrap(), client_dir.path().to_str().unwrap(), rpc);
    let node = format!("{}/octez-node", root_dir.to_str().unwrap());
    let jstz = format!("{}/scripts/jstz.sh", root_dir.to_str().unwrap());

    let kernel = format!("{}/target/kernel/jstz_kernel_installer.hex", root_dir.to_str().unwrap());
    let preimages = format!("{}/target/kernel/preimages", root_dir.to_str().unwrap());

    fs::create_dir_all(&log_dir).expect("Failed to create log directory");

    let mut children: Vec<Child> = Vec::new();

    /*let child = start_sandboxed_node(&node, &node_dir.path().to_path_buf(), port, rpc, &script_dir.to_path_buf()).unwrap();
    children.push(child);

    let child = init_sandboxed_client(&client, &script_dir.to_path_buf(), &node_dir.path().to_path_buf());
    //children.push(child);

    let child = start_rollup_node(&client, &kernel, &preimages, &rollup_node, &rollup_node_dir.path().to_path_buf(), &log_dir);
    children.push(child);*/

    // Get the path to the current executable
    let current_exe = env::current_exe().expect("Failed to get current executable path");

    // Start the sandboxed node using the CLI
    let child_node = Command::new("target/debug/sandbox_initializer")
        .args(&["start-node", "--node", &node, "--node-dir", node_dir.path().to_str().unwrap(), "--port", &port.to_string(), "--rpc", &rpc.to_string(), "--script-dir", script_dir.to_str().unwrap()])
        .spawn()
        .expect("Failed to start sandboxed node using CLI");
    println!("Started sandboxed node with PID: {}", child_node.id());
    children.push(child_node);

    // Initialize the sandboxed client using the CLI
    let child_client = Command::new("target/debug/sandbox_initializer")
        .args(&["init-client", "--client", &client, "--script-dir", script_dir.to_str().unwrap(), "--node-dir", node_dir.path().to_str().unwrap()])
        .spawn()
        .expect("Failed to initialize sandboxed client using CLI");
    println!("Initialized sandboxed client with PID: {}", child_client.id());
    children.push(child_client);

    // Start the rollup node using the CLI
    let child_rollup = Command::new("target/debug/sandbox_initializer")
        .args(&["start-rollup", "--client", &client, "--kernel", &kernel, "--preimages", &preimages, "--rollup-node", &rollup_node, "--rollup-node-dir", rollup_node_dir.path().to_str().unwrap(), "--log-dir", log_dir.to_str().unwrap()])
        .spawn()
        .expect("Failed to start rollup node using CLI");
    println!("Started rollup node with PID: {}", child_rollup.id());
    children.push(child_rollup);

    // Store the PIDs of the processes in the config
    let pids: Vec<u32> = children.iter().map(|child| child.id()).collect();
    for pid in &pids {
        cfg.add_pid(*pid).unwrap();
    }

    println!("export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y ;");
    println!("export OCTEZ_CLIENT_DIR={} ;", client_dir.path().to_str().unwrap());
    println!("export OCTEZ_NODE_DIR={} ;", node_dir.path().to_str().unwrap());
    println!("export OCTEZ_ROLLUP_DIR={} ;", rollup_node_dir.path().to_str().unwrap());
    println!("alias octez-client=\"{}\" ;", client);
    println!("alias jstz=\"{}\" ;", jstz);
    // TODO: The octez-reset alias is more complex and might need a separate script or function

    println!("The node, baker, and rollup node are now initialized. In the rest of this shell session, you may now run `octez-client` to communicate with the launched node. For instance:");
    println!("    octez-client rpc get /chains/main/blocks/head/metadata");
    println!("You may observe the logs of the node, baker, rollup node, and jstz kernel in `logs`. For instance:");
    println!("    tail -f logs/kernel.log");
    println!("To stop the node, baker, and rollup node you may run `octez-reset`.");
    println!("Additionally, you may now use `jstz` to run jstz-specific commands. For instance:");
    println!("    jstz deploy-bridge sr1..");
    println!("Warning: All aliases will be removed when you close this shell.");

    // Update the is_sandbox_running property
    cfg.set_is_sandbox_running(true).unwrap();
}

pub fn sandbox_stop(mut cfg: Config) {
    // Check if sandbox is not running
    if !cfg.get_is_sandbox_running().unwrap_or(false) {
        println!("Error: Sandbox is not running!");
        return;
    }

    // Kill the processes using their PIDs
    let pids = cfg.get_active_pids().unwrap();
    for pid in pids {
        if let Ok(mut child) = std::process::Child::try_wait(pid) {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        cfg.remove_pid(pid).unwrap();
    }

    // Update the is_sandbox_running property
    cfg.set_is_sandbox_running(false).unwrap();
}