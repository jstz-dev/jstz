use crate::deploy_bridge::deploy_bridge;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use crate::config::Config;
use crate::utils::handle_output;
use fs_extra::dir::{self, CopyOptions};

fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
    let cfg = Config::load_from_file().expect("Failed to load the config.");
    let mut cli_command = if command == "node" {
        cfg.octez_node_command()
    } else {
        cfg.octez_client_command()
    };

    let output = cli_command
        .args(args)
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

pub fn start_sandboxed_node(
    node_dir: &PathBuf,
    port: u16,
    rpc: u16,
    script_dir: &PathBuf,
) -> Result<Child, String> {
    // Initialize node config
    run_command(
        "node",
        &[
            "config",
            "init",
            "--network",
            "sandbox",
            "--data-dir",
            &node_dir.to_str().unwrap(),
            "--net-addr",
            &format!("127.0.0.1:{}", port),
            "--rpc-addr",
            &format!("127.0.0.1:{}", rpc),
            "--connections",
            "0",
        ],
    )?;

    // Generate an identity of the node we want to run
    run_command(
        "node",
        &[
            "identity",
            "generate",
            "--data-dir",
            &node_dir.to_str().unwrap(),
        ],
    )?;

    let cfg = Config::load_from_file().expect("Failed to load the config.");

    // Start newly configured node in the background
    let child = cfg
        .octez_node_command()
        .args(&[
            "run",
            "--synchronisation-threshold",
            "0",
            "--network",
            "sandbox",
            "--data-dir",
            &node_dir.to_str().unwrap(),
            "--sandbox",
            &format!("{}/sandbox.json", script_dir.to_str().unwrap()),
        ])
        .spawn()
        .expect("Failed to start node");

    Ok(child)
}

fn run_command_silently(args: &[&str]) -> bool {
    let cfg = Config::load_from_file().expect("Failed to load the config.");

    let output = cfg.octez_client_command().args(args).output();

    handle_output(&output);

    match output {
        Ok(o) => o.status.success(),
        Err(e) => {
            eprintln!("Error executing command: {}", e);
            return false;
        }
    }
}

fn wait_for_node_to_initialize() {
    if run_command_silently(&["rpc", "get", "/chains/main/blocks/head/hash"]) {
        return;
    }

    print!("Waiting for node to initialize...");
    while !run_command_silently(&["rpc", "get", "/chains/main/blocks/head/hash"]) {
        sleep(Duration::from_secs(1));
    }
}

pub fn init_sandboxed_client(client: &str, script_dir: &PathBuf, tx: Sender<&str>) {
    wait_for_node_to_initialize();

    run_command(client, &["bootstrapped"]).expect("Failed to bootstrap client");

    // Add bootstrapped identities
    run_command(
        client,
        &[
            "import",
            "secret",
            "key",
            "activator",
            "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
        ],
    )
    .expect("Failed to import activator key");

    // Activate alpha
    run_command(
        client,
        &[
            "-block",
            "genesis",
            "activate",
            "protocol",
            "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            "with",
            "fitness",
            "1",
            "and",
            "key",
            "activator",
            "and",
            "parameters",
            &format!("{}/sandbox-params.json", script_dir.to_str().unwrap()),
        ],
    )
    .expect("Failed to activate alpha");

    // Add more bootstrapped accounts
    let keys = [
        "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
        "edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
        "edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
        "edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
    ];

    for (i, key) in keys.iter().enumerate() {
        let account_name = format!("bootstrap{}", i + 1);
        run_command(
            client,
            &[
                "import",
                "secret",
                "key",
                &account_name,
                &format!("unencrypted:{}", key),
            ],
        )
        .expect(&format!("Failed to import {} key", account_name));
    }

    // Communicate the the node was activated to the other thread
    tx.send("activated").unwrap();

    // Continuously bake
    loop {
        if !run_command_silently(&["bake", "for", "--minimal-timestamp"]) {
            break;
        }
        sleep(Duration::from_secs(1));
    }
}

fn copy_directory_contents(src: &Path, dest: &Path) -> std::io::Result<()> {
    // Check if the source is a directory
    if !src.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Source is not a directory",
        ));
    }

    // Get the list of entries in the source directory
    let entries = std::fs::read_dir(src)?;

    let mut options = CopyOptions::new();
    options.overwrite = true; // Overwrite if destination exists

    // Iterate over each entry and copy to the destination
    for entry in entries {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.path().is_dir() {
            dir::copy(&entry.path(), &dest_path, &options).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;
        } else {
            std::fs::copy(&entry.path(), &dest_path)?;
        }
    }

    Ok(())
}

fn originate_rollup(
    kernel: &str,
    rollup_node_dir: &PathBuf,
    preimages: &PathBuf,
    rx: Receiver<&str>,
) -> Result<String, String> {
    println!("Waiting for node to activate...");
    rx.recv().expect("Failed to received the message.");
    println!("Node activated.");

    sleep(Duration::from_secs(1));

    // Originate rollup
    let cfg = Config::load_from_file().expect("Failed to load the config.");

    let output = cfg
        .octez_client_command()
        .args(&[
            "originate",
            "smart",
            "rollup",
            "jstz_rollup",
            "from",
            "bootstrap1",
            "of",
            "kind",
            "wasm_2_0_0",
            "of",
            "type",
            "(pair bytes (ticket unit))",
            "with",
            "kernel",
            &format!("file:{}", kernel),
            "--burn-cap",
            "999",
        ])
        .output()
        .expect("Failed to originate rollup");

    let output_result: Result<String, String> = if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };

    // Copy kernel installer preimages to rollup node directory
    let dest_dir = rollup_node_dir.join("wasm_2_0_0");
    fs::create_dir_all(&dest_dir).expect("Failed to create directory");

    copy_directory_contents(&preimages, &dest_dir)
        .expect("Failed to copy directory contents.");

    // Return address
    output_result
}

pub fn start_rollup_node(
    kernel: &str,
    preimages: &str,
    rollup_node_dir: &PathBuf,
    log_dir: &PathBuf,
    rx: Receiver<&str>,
) {
    let output = originate_rollup(kernel, rollup_node_dir, &PathBuf::from(preimages), rx);

    let cfg = Config::load_from_file().expect("Failed to load the config.");

    println!("Originating rollup");

    let mut address: String = Default::default();
    for line in output.unwrap().lines() {
        if line.contains("Address:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(value) = parts.iter().find(|&&word| word.starts_with("sr1")) {
                println!("Found address: {}", value);
                address = value.to_string();
            }
        }
    }

    let handle = thread::spawn({
        move || {
            thread::sleep(Duration::from_secs(1));
            let cfg = Config::load_from_file().expect("Failed to load the config.");
            println!("Deploying bridge:");
            deploy_bridge(address, &cfg);
        }
    });

    cfg.octez_rollup_node_command()
        .args(&[
            "run",
            "operator",
            "for",
            "jstz_rollup",
            "with",
            "operators",
            "bootstrap2",
            "--data-dir",
            rollup_node_dir.to_str().unwrap(),
            "--log-kernel-debug",
            "--log-kernel-debug-file",
            &format!("{}/kernel.log", log_dir.to_str().unwrap()),
        ])
        .output()
        .expect("Failed to start rollup node");

    handle.join().unwrap();
}
