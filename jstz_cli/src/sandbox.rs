use std::process::{Command, Child};
use std::env;
use std::path::PathBuf;
use tempfile::Builder;
use std::fs::File;
use std::fs;
use std::thread::sleep;
use std::time::Duration;
use std::path::Path;

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
    let mut count = 0;
    while !run_command_silently(client, &["rpc", "get", "/chains/main/blocks/head/hash"]) {
        count += 1;
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

fn start_rollup_node(rollup_node: &str, rollup_node_dir: &PathBuf, log_dir: &PathBuf) -> Child {
    originate_rollup(client, kernel, rollup_node_dir, preimages);

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
    //TODO: rollup_pids+=("$!")


    ctrlc::set_handler(move || {
        child.kill().expect("Failed to kill rollup node");
        std::process::exit(1);
    }).expect("Error setting Ctrl-C handler");

    child
}

pub fn sandbox_start() {
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
    env::set_var("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");

    let client = format!("{}/octez-client -base-dir {} -endpoint http://127.0.0.1:{}", root_dir.to_str().unwrap(), client_dir.path().to_str().unwrap(), rpc);
    let rollup_node = format!("{}/octez-smart-rollup-node -base-dir {} -endpoint http://127.0.0.1:{}", root_dir.to_str().unwrap(), client_dir.path().to_str().unwrap(), rpc);
    let node = format!("{}/octez-node", root_dir.to_str().unwrap());
    let jstz = format!("{}/scripts/jstz.sh", root_dir.to_str().unwrap());

    let kernel = format!("{}/target/kernel/jstz_kernel_installer.hex", root_dir.to_str().unwrap());
    let preimages = format!("{}/target/kernel/preimages", root_dir.to_str().unwrap());

    fs::create_dir_all(&log_dir).expect("Failed to create log directory");

    let mut children: Vec<Child> = Vec::new();

    let child = start_sandboxed_node(&node, &node_dir);
    children.push(child);

    let child = init_sandboxed_client(&client, &script_dir, &node_dir);
    children.push(child);

    let child = start_rollup_node(&rollup_node, &rollup_node_dir, &log_dir);
    children.push(child);

    println!("export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y ;");
    println!("export OCTEZ_CLIENT_DIR={} ;", client_dir.to_str().unwrap());
    println!("export OCTEZ_NODE_DIR={} ;", node_dir.to_str().unwrap());
    println!("export OCTEZ_ROLLUP_DIR={} ;", rollup_node_dir.to_str().unwrap());
    println!("alias octez-client=\"{}\" ;", client);
    println!("alias jstz=\"{}\" ;", jstz);
    // TODO: The octez-reset alias is more complex and might need a separate script or function

    eprintln!("The node, baker, and rollup node are now initialized. In the rest of this shell session, you may now run `octez-client` to communicate with the launched node. For instance:");
    eprintln!("    octez-client rpc get /chains/main/blocks/head/metadata");
    eprintln!("You may observe the logs of the node, baker, rollup node, and jstz kernel in `logs`. For instance:");
    eprintln!("    tail -f logs/kernel.log");
    eprintln!("To stop the node, baker, and rollup node you may run `octez-reset`.");
    eprintln!("Additionally, you may now use `jstz` to run jstz-specific commands. For instance:");
    eprintln!("    jstz deploy-bridge sr1..");
    eprintln!("Warning: All aliases will be removed when you close this shell.");
}

pub fn sandbox_stop(){

}