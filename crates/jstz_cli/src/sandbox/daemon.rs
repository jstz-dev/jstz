use anyhow::Result;
use daemonize::Daemonize;
use indicatif::{ProgressBar, ProgressStyle};
use jstz_node::{
    run_node, DEFAULT_KERNEL_FILE_PATH, DEFAULT_ROLLUP_NODE_RPC_ADDR,
    DEFAULT_ROLLUP_RPC_PORT,
};
use jstz_rollup::{
    deploy_ctez_contract, rollup::make_installer, BootstrapAccount, BridgeContract,
    JstzRollup,
};
use octez::OctezThread;
use regex::Regex;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::{self, sleep},
    time::Duration,
};

use anyhow::Result;
use console::style;
use jstz_node::run_node;
use jstz_rollup::{
    deploy_ctez_contract, rollup::make_installer, BootstrapAccount, BridgeContract,
    JstzRollup,
};
use log::{debug, info};
use octez::OctezThread;
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};
use tempfile::TempDir;
use tokio::task;

use crate::{
    config::{jstz_home_dir, Config, SandboxConfig},
    error::{bail_user_error, Result},
    sandbox::{
        SANDBOX_JSTZ_NODE_PORT, SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_PORT,
        SANDBOX_OCTEZ_NODE_RPC_PORT, SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    },
    term::styles,
};

fn octez_node_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_PORT
    )
}

fn octez_smart_rollup_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_SMART_ROLLUP_PORT
    )
}

fn jstz_node_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_LOCAL_HOST_ADDR, SANDBOX_JSTZ_NODE_PORT
    )
}

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

const SANDBOX_BANNER: &str = r#"
           __________
           \  jstz  /
            )______(
            |""""""|_.-._,.---------.,_.-._
            |      | | |               | | ''-.
            |      |_| |_             _| |_..-'
            |______| '-' `'---------'` '-'
            )""""""(
           /________\
           `'------'`
         .------------.
        /______________\
"#;

struct SandboxBootstrapAccount<'a> {
    address: &'a str,
    secret: &'a str,
}

const SANDBOX_BOOTSTRAP_ACCOUNT_XTZ_AMOUNT: u64 = 4000000000000;
const SANDBOX_BOOTSTRAP_ACCOUNT_CTEZ_AMOUNT: u64 = 100000000000;
const SANDBOX_BOOTSTRAP_ACCOUNTS: [SandboxBootstrapAccount; 5] = [
    SandboxBootstrapAccount {
        address: "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
        secret: "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
    },
    SandboxBootstrapAccount {
        address: "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        secret: "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
    },
    SandboxBootstrapAccount {
        address: "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU",
        secret: "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
    },
    SandboxBootstrapAccount {
        address: "tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv",
        secret: "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
    },
    SandboxBootstrapAccount {
        address: "tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv",
        secret: "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
    },
];

const ACTIVATOR_ACCOUNT_ALIAS: &str = "activator";
fn sandbox_daemon_log_path() -> Result<PathBuf> {
    Ok(logs_dir()?.join("sandbox_daemon.log"))
}

const ACTIVATOR_ACCOUNT_SK: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";

const OPERATOR_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"; // bootstrap1

fn ctez_bootstrap_accounts() -> Vec<BootstrapAccount> {
    SANDBOX_BOOTSTRAP_ACCOUNTS
        .iter()
        .map(|account| BootstrapAccount {
            address: account.address.to_string(),
            amount: SANDBOX_BOOTSTRAP_ACCOUNT_CTEZ_AMOUNT,
        })
        .collect::<Vec<BootstrapAccount>>()
}

fn cached_identity_path() -> PathBuf {
    jstz_home_dir().join("octez-node-identity.json")
}

fn octez_node_identity_path(cfg: &Config) -> Result<PathBuf> {
    Ok(cfg.octez_node()?.octez_node_dir.join("identity.json"))
}

fn generate_identity(cfg: &Config) -> Result<()> {
    let cached_identity_path = cached_identity_path();
    let octez_node_identity_path = octez_node_identity_path(cfg)?;

    if cached_identity_path.exists() {
        debug!("Cached identity hit");
        fs::copy(cached_identity_path, octez_node_identity_path)?;
        return Ok(());
    }

    debug!("Cached identity miss");
    debug!("Generating identity...");
    cfg.octez_node()?.generate_identity()?;
    debug!("Identity generated");

    fs::copy(octez_node_identity_path, cached_identity_path)?;
    debug!("Cached identity");

    Ok(())
}

const MAX_PROGRESS: u32 = 17;
fn progress_step(progress: &mut u32) {
    *progress += 1;
    print!("({})", *progress);
}

fn init_node(progress: &mut u32, cfg: &Config) -> Result<()> {
    // 1. Initialize the octez-node configuration
    debug!("Initializing octez-node");

    cfg.octez_node()?.config_init(
        "sandbox",
        &format!("{}:{}", SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_PORT),
        &format!(
            "{}:{}",
            SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_RPC_PORT
        ),
        0,
    )?;
    debug!("\tInitialized octez-node configuration");

    // 2. Generate an identity
    generate_identity(cfg)?;

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
        .octez_client_sandbox()?
        .rpc(&["get", "/chains/main/blocks/head/hash"])
        .is_ok())
}

fn wait_for_node_to_initialize(cfg: &Config) -> Result<()> {
    if is_node_running(cfg)? {
        return Ok(());
    }

    debug!("Waiting for node to initialize...");
    while !is_node_running(cfg)? {
        sleep(Duration::from_secs(1));
    }

    debug!("Node initialized");
    Ok(())
}

fn init_client(cfg: &Config) -> Result<()> {
    // 1. Wait for the node to initialize
    wait_for_node_to_initialize(cfg)?;

    // 2. Wait for the node to be bootstrapped
    debug!("Waiting for node to bootstrap...");
    cfg.octez_client_sandbox()?.wait_for_node_to_bootstrap()?;
    debug!(" done");

    // 3. Import activator and bootstrap accounts
    debug!("Importing activator account...");
    cfg.octez_client_sandbox()?
        .import_secret_key(ACTIVATOR_ACCOUNT_ALIAS, ACTIVATOR_ACCOUNT_SK)?;
    debug!("done");

    // 4. Activate alpha
    debug!("Activating alpha...");
    cfg.octez_client_sandbox()?.activate_protocol(
        "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        "1",
        "activator",
        SANDBOX_PARAMS_PATH,
    )?;
    debug!(" done");

    // 5. Import bootstrap accounts
    for (i, bootstrap_account) in SANDBOX_BOOTSTRAP_ACCOUNTS.iter().enumerate() {
        let name = format!("bootstrap{}", i + 1);
        cfg.octez_client_sandbox()?
            .import_secret_key(&name, bootstrap_account.secret)?;
        debug!(
            "Imported account {}. address: {}, secret: {}",
            name, bootstrap_account.address, bootstrap_account.secret
        );
    }

    Ok(())
}

fn client_bake(cfg: &Config, log_file: &File) -> Result<()> {
    // SAFETY: When a baking fails, then we want to silently ignore the error and
    // try again later since the `client_bake` function is looped in the `OctezThread`.
    let _ = cfg
        .octez_client_sandbox()?
        .bake(log_file, &["for", "--minimal-timestamp"]);
    Ok(())
}

/// Since actix_web uses a single-threaded runtime,
/// the tasks spawned by `jstz_node` expect to run on the same thread.
/// For more information, see: https://docs.rs/actix-rt/latest/actix_rt/
async fn run_jstz_node() -> Result<()> {
    let local = task::LocalSet::new();

    local
        .run_until(async {
            task::spawn_local(async {
                debug!("Jstz node started 🎉");

                jstz_node::run(
                    SANDBOX_LOCAL_HOST_ADDR,
                    SANDBOX_JSTZ_NODE_PORT,
                    &format!(
                        "http://{}:{}",
                        SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_SMART_ROLLUP_PORT
                    ),
                    &logs_dir()?.join("kernel.log"),
                )
                .await
            })
            .await
        })
        .await??;

    Ok(())
}

fn start_sandbox(cfg: &Config) -> Result<(OctezThread, OctezThread, OctezThread)> {
    // 1. Init node
    init_node(progress, cfg)?;

    // 2. As a thread, start node
    let node = OctezThread::from_child(start_node(cfg)?);
    debug!("Started octez-node");

    // 3. Init client
    init_client(cfg)?;
    debug!("Initialized octez-client");

    // 4. As a thread, start baking
    let client_logs = File::create(client_log_path()?)?;
    let baker = OctezThread::new(cfg.clone(), move |cfg| {
        client_bake(cfg, &client_logs)?;
        Ok(())
    });
    debug!("Started baker (using octez-client)");

    // 5. Deploy bridge
    debug!("Deploying bridge...");

    let ctez_address = deploy_ctez_contract(
        &cfg.octez_client_sandbox()?,
        OPERATOR_ADDRESS,
        ctez_bootstrap_accounts(),
    )?;

    let bridge = BridgeContract::deploy(
        &cfg.octez_client_sandbox()?,
        OPERATOR_ADDRESS,
        &ctez_address,
    )?;

    debug!("Bridge deployed at {}", bridge);

    // 6. Create an installer kernel
    progress_step(progress);
    print!("Creating installer kernel...");

    let preimages_dir = TempDir::with_prefix("jstz_sandbox_preimages")?.into_path();

    let installer = make_installer(Path::new(JSTZ_KERNEL_PATH), &preimages_dir, &bridge)?;
    debug!(
        "Installer kernel created with preimages at {:?}",
        preimages_dir
    );

    // 7. Originate the rollup
    let rollup =
        JstzRollup::deploy(&cfg.octez_client_sandbox()?, OPERATOR_ADDRESS, &installer)?;

    debug!("`jstz_rollup` originated at {}", rollup);

    // 8. As a thread, start rollup node
    debug!("Starting rollup node...");

    let logs_dir = logs_dir()?;
    let rollup_node = OctezThread::from_child(rollup.run(
        &cfg.octez_rollup_node_sandbox()?,
        OPERATOR_ADDRESS,
        &preimages_dir,
        &logs_dir,
        SANDBOX_LOCAL_HOST_ADDR,
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    )?);
    debug!("Started octez-smart-rollup-node");

    // 9. Set the rollup address in the bridge
    bridge.set_rollup(&cfg.octez_client_sandbox()?, OPERATOR_ADDRESS, &rollup)?;
    debug!("`jstz_bridge` `rollup` address set to {}", rollup);

    Ok((node, baker, rollup_node))
}

fn format_sandbox_bootstrap_accounts() -> Table {
    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Address"),
        Cell::new("XTZ Balance"),
        Cell::new("CTEZ Balance"),
    ]));

    for (i, bootstrap_account) in SANDBOX_BOOTSTRAP_ACCOUNTS.iter().enumerate() {
        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "(bootstrap{}) {}",
                i + 1,
                bootstrap_account.address
            )),
            Cell::new(&SANDBOX_BOOTSTRAP_ACCOUNT_XTZ_AMOUNT.to_string()),
            Cell::new(&SANDBOX_BOOTSTRAP_ACCOUNT_CTEZ_AMOUNT.to_string()),
        ]));
    }

    table
}

pub async fn main(cfg: &mut Config) -> Result<()> {
    // 1. Check if sandbox is already running
    if cfg.sandbox.is_some() {
        bail_user_error!("The sandbox is already running!");
    }

    // Print banner
    info!("{}", style(SANDBOX_BANNER).bold());
    info!(
        "        {} {}",
        env!("CARGO_PKG_VERSION"),
        styles::url(env!("CARGO_PKG_REPOSITORY"))
    );
    info!("");

    // 1. Configure sandbox
    debug!("Configuring sandbox...");
    let sandbox_cfg = SandboxConfig {
        pid: std::process::id(),
        octez_client_dir: TempDir::with_prefix("octez_client")?.into_path(),
        octez_node_dir: TempDir::with_prefix("octez_node")?.into_path(),
        octez_rollup_node_dir: TempDir::with_prefix("octez_rollup_node")?.into_path(),
    };

    cfg.sandbox = Some(sandbox_cfg);
    debug!("Sandbox configured {:?}", cfg.sandbox);

    // 2. Start sandbox
    let (node, baker, rollup_node) = start_sandbox(cfg)?;
    debug!("Sandbox started 🎉");

    // 3. Save config
    debug!("Saving sandbox config");
    cfg.save()?;

    info!(
        "octez-node is listening on: {}",
        styles::url(octez_node_endpoint())
    );
    info!(
        "octez-smart-rollup-node is listening on: {}",
        styles::url(octez_smart_rollup_endpoint())
    );
    info!(
        "jstz-node is listening on: {}",
        styles::url(jstz_node_endpoint())
    );

    info!("\nTezos bootstrap accounts:");

    let mut sandbox_bootstrap_accounts = format_sandbox_bootstrap_accounts();
    sandbox_bootstrap_accounts.set_format({
        let mut format = *FORMAT_DEFAULT;
        format.indent(2);
        format
    });

    info!("{}", sandbox_bootstrap_accounts);

    // 4. Wait for the sandbox or jstz-node to shutdown (either by the user or by an error)
    run_jstz_node().await?;
    OctezThread::join(vec![baker, rollup_node, node])?;

    cfg.sandbox = None;
    cfg.save()?;
    Ok(())
}

pub async fn main(no_daemon: bool, cfg: &mut Config) -> Result<()> {
    if no_daemon {
        run_sandbox(cfg).await?;
    } else {
        let path = sandbox_daemon_log_path()?;
        let stdout_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.clone())?;

        let mut child = Command::new(std::env::current_exe()?)
            .args(&["sandbox", "start", "--no-daemon"])
            .stdout(Stdio::from(stdout_file))
            .spawn()?;

        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();

        let regex = Regex::new(r"\((\d+)\)").unwrap();

        let mut progress: u32 = 0;

        let progress_bar = ProgressBar::new(MAX_PROGRESS as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar().template(
                "{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
            )?,
        );

        loop {
            reader.seek(SeekFrom::Current(0))?;

            while reader.read_line(&mut buffer)? > 0 {
                if let Some(captures) = regex.captures(&buffer) {
                    if let Some(matched) = captures.get(1) {
                        if let Ok(num) = matched.as_str().parse::<u32>() {
                            progress = num;
                            progress_bar.set_position(progress.into());
                        }
                    }
                }
                buffer.clear();
            }

            if progress == MAX_PROGRESS {
                progress_bar.finish_with_message("Sandbox started 🎉");
                break;
            }

            if let Ok(Some(status)) = child.try_wait() {
                progress_bar
                    .finish_with_message(format!("Sandbox failed to start: {:}", status));
                break;
            }

            thread::sleep(Duration::from_millis(1000));
        }
    }
    Ok(())
}
