use std::{
    fs::{self, File},
    io::Write,
    process::{Child, Command},
    sync::mpsc::{self, Sender},
    thread::{self, sleep, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use fs_extra::dir::CopyOptions;
use jstz_core::kv::value::serialize;
use nix::libc::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use tempfile::TempDir;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup_installer_config::yaml::{Instr, SetArgs, YamlConfig};

use crate::{
    bridge,
    config::{Config, SandboxConfig, SANDBOX_OCTEZ_SMART_ROLLUP_PORT},
};

fn init_node(cfg: &Config) -> Result<()> {
    // 1. Initialize the octez-node configuration
    print!("Initializing octez-node configuration...");
    cfg.octez_node()?.config_init(
        "sandbox",
        &format!("127.0.0.1:{}", cfg.sandbox()?.octez_node_port),
        &format!("127.0.0.1:{}", cfg.sandbox()?.octez_node_rpc_port),
        0,
    )?;
    println!(" done");

    // 2. Generate an identity
    print!("Generating identity...");
    cfg.octez_node()?.generate_identity()?;
    println!(" done");
    Ok(())
}

fn start_node(cfg: &Config) -> Result<Child> {
    // Run the octez-node in sandbox mode
    let sandbox_file = cfg.jstz_path.join("crates/jstz_cli/sandbox.json");
    let log_file = File::create(cfg.jstz_path.join("logs/node.log"))?;

    cfg.octez_node()?.run(
        &log_file,
        &[
            "--synchronisation-threshold",
            "0",
            "--network",
            "sandbox",
            "--sandbox",
            sandbox_file.to_str().expect("Invalid path"),
        ],
    )
}

fn is_node_running(cfg: &Config) -> Result<bool> {
    Ok(cfg
        .octez_client()?
        .rpc(&["get", "/chains/main/blocks/head/hash"])
        .is_ok())
}

fn wait_for_node_to_initialize(cfg: &Config) -> Result<()> {
    if is_node_running(cfg)? {
        return Ok(());
    }

    print!("Waiting for node to initialize...");
    while !is_node_running(cfg)? {
        sleep(Duration::from_secs(1));
        print!(".")
    }

    println!(" done");
    Ok(())
}

const ACTIVATOR_ACCOUNT_SK: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";

const BOOTSTRAP_ACCOUNT_SKS: [&str; 5] = [
    "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
    "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
    "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
    "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
    "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
];

fn init_client(cfg: &Config) -> Result<()> {
    // 1. Wait for the node to initialize
    wait_for_node_to_initialize(cfg)?;

    // 2. Wait for the node to be bootstrapped
    print!("Waiting for node to bootstrap...");
    cfg.octez_client()?.wait_for_node_to_bootstrap()?;
    println!(" done");

    // 3. Import activator and bootstrap accounts
    print!("Importing activator account...");
    cfg.octez_client()?
        .import_secret_key("activator", ACTIVATOR_ACCOUNT_SK)?;
    println!(" done");

    // 4. Activate alpha
    let sandbox_params_file = cfg.jstz_path.join("crates/jstz_cli/sandbox-params.json");

    print!("Activating alpha...");
    cfg.octez_client()?.activate_protocol(
        "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        "1",
        "activator",
        sandbox_params_file.to_str().expect("Invalid path"),
    )?;
    println!(" done");

    // 5. Import bootstrap accounts
    for (i, sk) in BOOTSTRAP_ACCOUNT_SKS.iter().enumerate() {
        let name = format!("bootstrap{}", i + 1);
        println!("Importing account {}:{}", name, sk);
        cfg.octez_client()?.import_secret_key(&name, sk)?
    }

    Ok(())
}

fn client_bake(cfg: &Config, log_file: &File) -> Result<()> {
    // SAFETY: When a baking fails, then we want to silently ignore the error and
    // try again later since the `client_bake` function is looped in the `OctezThread`.
    let _ = cfg
        .octez_client()?
        .bake(log_file, &["for", "--minimal-timestamp"]);
    Ok(())
}

fn originate_rollup(cfg: &Config) -> Result<String> {
    let target = cfg.jstz_path.join("target/kernel");

    // 1. Originate the rollup
    let kernel_file = target.join("jstz_kernel_installer.hex");

    let address = cfg.octez_client()?.originate_rollup(
        "bootstrap1",
        "jstz_rollup",
        "wasm_2_0_0",
        "(pair bytes (ticket unit))",
        &format!("file:{}", kernel_file.to_str().expect("Invalid path")),
    )?;

    // 2. Copy kernel installer preimages to rollup node directory
    let rollup_node_preimages_dir =
        &cfg.sandbox()?.octez_rollup_node_dir.join("wasm_2_0_0");

    fs::create_dir_all(rollup_node_preimages_dir)?;
    fs_extra::dir::copy(
        target.join("preimages/"),
        rollup_node_preimages_dir,
        &CopyOptions {
            content_only: true,
            ..Default::default()
        },
    )?;

    Ok(address)
}

fn start_rollup_node(cfg: &Config) -> Result<Child> {
    let rollup_log_file = File::create(cfg.jstz_path.join("logs/rollup.log"))?;
    let kernel_log_file = cfg.jstz_path.join("logs/kernel.log");

    cfg.octez_rollup_node()?.run(
        "127.0.0.1",
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
        &rollup_log_file,
        "jstz_rollup",
        "bootstrap2",
        &[
            "--log-kernel-debug",
            "--log-kernel-debug-file",
            kernel_log_file.to_str().expect("Invalid path"),
        ],
    )
}

fn smart_rollup_installer(cfg: &Config, bridge_address: &str) -> Result<()> {
    //Convert address
    let bridge_address = ContractKt1Hash::from_base58_check(bridge_address)?;

    let instructions = YamlConfig {
        instructions: vec![Instr::Set(SetArgs {
            value: hex::encode(
                serialize(&bridge_address).expect("Could not serialize address"),
            ),
            to: "/ticketer".to_owned(),
        })],
    };
    let yaml_config = serde_yaml::to_string(&instructions).unwrap();

    // Create a temporary file for the serialized representation of the address computed by octez-codec
    let mut temp_file = tempfile::NamedTempFile::new()?;
    temp_file.write_all(yaml_config.as_bytes())?;

    // Get the path to the temporary file if needed later in the code
    let setup_file_path = temp_file.path().to_owned();

    // Create an installer kernel
    let mut installer_command = Command::new("smart-rollup-installer");

    installer_command.args([
        "get-reveal-installer",
        "--setup-file",
        &setup_file_path.to_str().expect("Invalid path"),
        "--output",
        cfg.jstz_path
            .join("target/kernel/jstz_kernel_installer.hex")
            .to_str()
            .expect("Invalid path"),
        "--preimages-dir",
        &cfg.jstz_path
            .join("target/kernel")
            .join("preimages/")
            .to_str()
            .expect("Invalid path"),
        "--upgrade-to",
        &cfg.jstz_path
            .join("target/wasm32-unknown-unknown/release/jstz_kernel.wasm")
            .to_str()
            .expect("Invalid path"),
    ]);

    let installer_output = installer_command.output()?;

    if !installer_output.status.success() {
        return Err(anyhow::anyhow!(
            "Command {:?} failed:\n {}",
            installer_command,
            String::from_utf8_lossy(&installer_output.stderr)
        ));
    }

    Ok(())
}

struct OctezThread {
    shutdown_tx: Sender<()>,
    thread_handle: JoinHandle<Result<()>>,
}

impl OctezThread {
    pub fn new<F>(cfg: &Config, f: F) -> Self
    where
        F: Fn(&Config) -> Result<()> + Send + 'static,
    {
        let cfg = cfg.clone();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let thread_handle: JoinHandle<Result<()>> = thread::spawn(move || {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                f(&cfg)?;

                sleep(Duration::from_secs(1));
            }

            Ok(())
        });

        Self {
            shutdown_tx,
            thread_handle,
        }
    }

    pub fn from_child(mut child: Child) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let thread_handle: JoinHandle<Result<()>> = thread::spawn(move || {
            loop {
                if child.try_wait()?.is_some() {
                    break;
                }

                if shutdown_rx.try_recv().is_ok() {
                    child.kill()?;
                    break;
                }

                sleep(Duration::from_secs(1));
            }

            Ok(())
        });

        Self {
            shutdown_tx,
            thread_handle,
        }
    }

    pub fn is_running(&self) -> bool {
        !self.thread_handle.is_finished()
    }

    pub fn shutdown(self) -> Result<()> {
        self.shutdown_tx.send(())?;
        match self.thread_handle.join() {
            Ok(result) => result?,
            Err(_) => {
                // thread paniced
            }
        };
        Ok(())
    }

    pub fn join(threads: Vec<Self>) -> Result<()> {
        let mut signals = Signals::new([SIGINT, SIGTERM])?;

        // Loop until 1 of the threads fails
        'main_loop: loop {
            for thread in threads.iter() {
                if !thread.is_running() {
                    break 'main_loop;
                }
            }

            for signal in signals.pending() {
                match signal {
                    SIGINT | SIGTERM => {
                        println!("Received signal {:?}, shutting down...", signal);
                        break 'main_loop;
                    }
                    _ => unreachable!(),
                }
            }
        }

        // Shutdown all running threads
        for thread in threads {
            if thread.is_running() {
                thread.shutdown()?;
            }
        }

        Ok(())
    }
}

fn start_sandbox(cfg: &Config) -> Result<(OctezThread, OctezThread, OctezThread)> {
    // 1. Init node
    init_node(cfg)?;

    // 2. As a thread, start node
    print!("Starting node...");
    let node = OctezThread::from_child(start_node(cfg)?);
    println!(" done");

    // 3. Init client
    init_client(cfg)?;
    println!("Client initialized");

    // 4. As a thread, start baking
    print!("Starting baker...");
    let client_logs = File::create(cfg.jstz_path.join("logs/client.log"))?;
    let baker = OctezThread::new(cfg, move |cfg| {
        client_bake(cfg, &client_logs)?;
        Ok(())
    });
    println!(" done");

    // 5. Deploy bridge
    println!("Deploying bridge...");
    let bridge_address = bridge::deploy(cfg)?;
    println!("\t`jstz_bridge` deployed at {}", bridge_address);

    // 6. Create an installer kernel
    print!("Creating installer kernel...");
    smart_rollup_installer(cfg, bridge_address.as_str())?;
    println!("done");

    // 7. Originate the rollup
    let rollup_address = originate_rollup(cfg)?;
    println!("`jstz_rollup` originated at {}", rollup_address);

    // 8. As a thread, start rollup node
    print!("Starting rollup node...");
    let rollup_node = OctezThread::from_child(start_rollup_node(cfg)?);
    println!(" done");

    bridge::set_rollup(cfg, &rollup_address)?;
    println!("\t`jstz_bridge` `rollup` address set to {}", rollup_address);

    println!("Bridge deployed");

    Ok((node, baker, rollup_node))
}

pub fn main(cfg: &mut Config) -> Result<()> {
    // 1. Check if sandbox is already running
    if cfg.sandbox.is_some() {
        return Err(anyhow::anyhow!("Sandbox is already running!"));
    }

    // 1. Configure sandbox
    print!("Configuring sandbox...");
    let sandbox_cfg = SandboxConfig::new(
        std::process::id(),
        TempDir::with_prefix("octez_client")?.into_path(),
        TempDir::with_prefix("octez_node")?.into_path(),
        TempDir::with_prefix("octez_rollup_node")?.into_path(),
    );

    // Create logs directory
    fs::create_dir_all(cfg.jstz_path.join("logs"))?;

    cfg.sandbox = Some(sandbox_cfg);
    println!(" done");

    // 2. Start sandbox
    let (node, baker, rollup_node) = start_sandbox(cfg)?;
    println!("Sandbox started 🎉");

    // 3. Save config
    println!("Saving sandbox config");
    cfg.save()?;

    // 4. Wait for the sandbox to shutdown (either by the user or by an error)
    OctezThread::join(vec![baker, rollup_node, node])?;

    cfg.sandbox = None;
    cfg.save()?;
    Ok(())
}
