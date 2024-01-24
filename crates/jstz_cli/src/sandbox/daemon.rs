use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::Child,
    thread::sleep,
    time::Duration,
};

use anyhow::Result;
use jstz_rollup::{
    deploy_ctez_contract, rollup::make_installer, BootstrapAccount, BridgeContract,
    JstzRollup,
};
use octez::OctezThread;
use tempfile::TempDir;

use crate::config::{Config, SandboxConfig, SANDBOX_OCTEZ_SMART_ROLLUP_PORT};

include!(concat!(env!("OUT_DIR"), "/sandbox_paths.rs"));

fn logs_dir() -> Result<PathBuf> {
    Ok(env::current_dir()?.join("logs"))
}

fn node_log_path() -> Result<PathBuf> {
    Ok(logs_dir()?.join("node.log"))
}

fn client_log_path() -> Result<PathBuf> {
    Ok(logs_dir()?.join("client.log"))
}

const ACTIVATOR_ACCOUNT_SK: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";

const BOOTSTRAP_ACCOUNT_SKS: [&str; 5] = [
    "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh", // bootstrap1
    "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo", // bootstrap2
    "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ", // bootstrap3
    "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3", // bootstrap4
    "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm", // bootstrap5
];

const OPERATOR_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"; // bootstrap1

// FIXME: Exposing this address is a hack (consistent with the current implementation)
// In future, we should permit users to configure the L1 client address they wish to use
pub(crate) const CLIENT_ADDRESS: &str = "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN"; // bootstrap2

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
    let log_file = File::create(node_log_path()?)?;

    cfg.octez_node()?.run(
        &log_file,
        &[
            "--synchronisation-threshold",
            "0",
            "--network",
            "sandbox",
            "--sandbox",
            SANDBOX_PATH,
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
    print!("Activating alpha...");
    cfg.octez_client()?.activate_protocol(
        "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        "1",
        "activator",
        SANDBOX_PARAMS_PATH,
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
    let client_logs = File::create(client_log_path()?)?;
    let baker = OctezThread::new(cfg.clone(), move |cfg| {
        client_bake(cfg, &client_logs)?;
        Ok(())
    });
    println!(" done");

    // 5. Deploy bridge
    print!("Deploying bridge...");

    let ctez_bootstrap_accounts = &[BootstrapAccount {
        address: String::from(CLIENT_ADDRESS),
        amount: 100000000,
    }];

    let ctez_address = deploy_ctez_contract(
        &cfg.octez_client()?,
        OPERATOR_ADDRESS,
        ctez_bootstrap_accounts.iter(),
    )?;

    let bridge =
        BridgeContract::deploy(&cfg.octez_client()?, OPERATOR_ADDRESS, &ctez_address)?;

    println!(" done");
    println!("\t`jstz_bridge` deployed at {}", bridge);

    // 6. Create an installer kernel
    print!("Creating installer kernel...");

    let preimages_dir = TempDir::with_prefix("jstz_sandbox_preimages")?.into_path();

    let installer = make_installer(Path::new(JSTZ_KERNEL_PATH), &preimages_dir, &bridge)?;
    println!("done");

    // 7. Originate the rollup
    let rollup = JstzRollup::deploy(&cfg.octez_client()?, OPERATOR_ADDRESS, &installer)?;

    println!("`jstz_rollup` originated at {}", rollup);

    // 8. As a thread, start rollup node
    print!("Starting rollup node...");
    let rollup_node = OctezThread::from_child(rollup.run(
        &cfg.octez_rollup_node()?,
        OPERATOR_ADDRESS,
        &preimages_dir,
        &logs_dir()?,
        "127.0.0.1",
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    )?);
    println!(" done");

    // 9. Set the rollup address in the bridge
    bridge.set_rollup(&cfg.octez_client()?, OPERATOR_ADDRESS, &rollup)?;
    println!("\t`jstz_bridge` `rollup` address set to {}", rollup);

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
    fs::create_dir_all(logs_dir()?)?;

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
