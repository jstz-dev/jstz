use std::path::PathBuf;
use std::fs;
use std::time::Duration;
use std::process::Child;
use std::thread::sleep;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{self, Sender, Receiver};
//use std::io::Write;
//use std::io::BufRead;
//use std::io::Seek;
//use std::io::SeekFrom;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::process::{Command};
use std::process::Stdio;


use crate::config::Config;
use crate::utils::handle_output;
use fs_extra::dir::{self, CopyOptions};

fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
    let mut cfg = Config::default();
    cfg.load_from_file();
    let mut cli_command = if command=="node" {cfg.octez_node_command()} else {cfg.octez_client_command()};

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

pub fn start_sandboxed_node(node: &str, node_dir: &PathBuf, port: u16, rpc: u16, script_dir: &PathBuf) -> Result<Child, String> {
    // Initialize node config
    run_command("node", &[
        "config", "init",
        "--network", "sandbox",
        "--data-dir", &node_dir.to_str().unwrap(),
        "--net-addr", &format!("127.0.0.1:{}", port),
        "--rpc-addr", &format!("127.0.0.1:{}", rpc),
        "--connections", "0"
    ])?;

    // Generate an identity of the node we want to run
    run_command("node", &[
        "identity", "generate",
        "--data-dir", &node_dir.to_str().unwrap()
    ])?;

    let mut cfg = Config::default();
    cfg.load_from_file();

    // Start newly configured node in the background
    let child = cfg.octez_node_command()
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
    let mut cfg = Config::default();
    cfg.load_from_file();

    let output = cfg.octez_client_command()
        .args(args)
        .output();
    
    handle_output(&output);

    match output {
        Ok(o) => {
            o.status.success()
        }
        Err(e) => {
            eprintln!("Error executing command: {}", e);
            return false;
        }
    }
}

fn wait_for_node_to_initialize(client: &str) {
    if run_command_silently(client, &["rpc", "get", "/chains/main/blocks/head/hash"]) {
        return;
    }

    print!("Waiting for node to initialize...");
    while !run_command_silently(client, &["rpc", "get", "/chains/main/blocks/head/hash"]) {
        sleep(Duration::from_secs(1));
    }
}

pub fn init_sandboxed_client(client: &str, script_dir: &PathBuf, node_dir: &PathBuf, tx: Sender<&str>) {
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

    // Communicate the the node was activated to the other thread
    tx.send("activated").unwrap();

    // Continuously bake
    loop {
        if !run_command_silently(client, &["bake", "for", "--minimal-timestamp"]) {
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
            dir::copy(&entry.path(), &dest_path, &options)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        } else {
            std::fs::copy(&entry.path(), &dest_path)?;
        }
    }

    Ok(())
}

fn originate_rollup(client: &str, kernel: &str, rollup_node_dir: &PathBuf, preimages: &PathBuf, rx: Receiver<&str>) {
    println!("Waiting for node to activate...");
    let message = rx.recv().unwrap();
    println!("Node activated.");

    sleep(Duration::from_secs(1));

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

    copy_directory_contents(&preimages, &dest_dir);
}

/// Pipe streams are blocking, we need separate threads to monitor them without blocking the primary thread.
/*fn child_stream_to_vec<R>(mut stream: R) -> Arc<Mutex<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    let out = Arc::new(Mutex::new(Vec::new()));
    let vec = out.clone();
    thread::Builder::new()
        .name("child_stream_to_vec".into())
        .spawn(move || loop {
            let mut buf = [0];
            match stream.read(&mut buf) {
                Err(err) => {
                    println!("{}] Error reading from stream: {}", line!(), err);
                    break;
                }
                Ok(got) => {
                    if got == 0 {
                        break;
                    } else if got == 1 {
                        vec.lock().expect("!lock").push(buf[0])
                    } else {
                        println!("{}] Unexpected number of bytes: {}", line!(), got);
                        break;
                    }
                }
            }
        })
        .expect("!thread");
    out
}*/


pub async fn start_rollup_node(client: &str, kernel: &str, preimages: &str, rollup_node: &str, rollup_node_dir: &PathBuf, log_dir: &PathBuf, rx: Receiver<&str>) {
    originate_rollup(client, kernel, rollup_node_dir, &PathBuf::from(preimages), rx);

    let mut cfg = Config::default();
    cfg.load_from_file();

    println!("rollup node run operator WOOHOO");

    let child = cfg.octez_rollup_node_command()
        .args(&[
            "run", "operator", "for", "jstz_rollup",
            "with", "operators", "bootstrap2",
            "--data-dir", rollup_node_dir.to_str().unwrap(),
            "--log-kernel-debug",
            "--log-kernel-debug-file", &format!("{}/kernel.log", log_dir.to_str().unwrap())
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start rollup node");

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    
    let mut lines = reader.lines();
    
    tokio::spawn(async move {
        let status = child.wait().await
            .expect("child process encountered an error");

        println!("child status was: {}", status);
    });

    while let Some(line) = lines.next_line().await.expect("Failed to read line") {
        println!("Found the line: {}", line);
        // Do something with the line
    }

    /*let output_file_path = "/child_output.txt";

    let mut child = cfg.octez_rollup_node_command()
        .args(&[
            "run", "operator", "for", "jstz_rollup",
            "with", "operators", "bootstrap2",
            "--data-dir", rollup_node_dir.to_str().unwrap(),
            "--log-kernel-debug",
            "--log-kernel-debug-file", &format!("{}/kernel.log", log_dir.to_str().unwrap())
        ])
        .stdout(File::create(&output_file_path).expect("Failed to open output file")) // Capture stdout
        .spawn()
        .expect("Failed to start rollup node");

    /*let stdout = child.stdout().take().expect("no stdout");

    let result: Vec<_> = BufReader::new(stdout)
        .lines()
        .inspect(|s| println!("> {:?}", s))
        .collect();

    println!("All the lines: {:?}", result);*/

    /*let reader = BufReader::new(child.stdout.take().expect("Failed to take child stdout"));

    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        println!("LINE LINE LINE: {}", line);
        if line.contains("of kind wasm_2_0_0") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let address = parts[parts.len() - 2]; // The address is the second last word in the line
                println!("Extracted address WOOOHOOO WOOOOOOHOOOOOOOO WOOOOOOHOOOOO: {}", address);
                break; // Exit the loop if you only want to capture the address once
            }
        }
    }*/
    let mut last_read_position = 0;

loop {
    let mut file = File::open(&output_file_path).expect("Failed to open output file for reading");
    file.seek(SeekFrom::Start(last_read_position)).expect("Failed to seek in file");

    let reader = BufReader::new(&file);
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        if line.contains("of kind wasm_2_0_0") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let address = parts[parts.len() - 2];
                println!("Extracted address: {}", address);
                return; // Exit the loop and function once the address is found
            }
        }
    }

    last_read_position = file.seek(SeekFrom::Current(0)).expect("Failed to get current file position");

    sleep(Duration::from_secs(1)); // Sleep for a second before checking again
}*/
}