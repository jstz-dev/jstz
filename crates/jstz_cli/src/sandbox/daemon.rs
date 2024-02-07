use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::Child,
    thread::sleep,
    time::Duration,
};

use console::style;
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
    config::{
        Config, SandboxConfig, SANDBOX_JSTZ_NODE_PORT, SANDBOX_OCTEZ_NODE_PORT,
        SANDBOX_OCTEZ_NODE_RPC_PORT, SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    },
    error::{bail_user_error, Result},
    term::styles,
};

const SANDBOX_JSTZ_NODE_ADDR: &str = "127.0.0.1";
const SANDBOX_OCTEZ_SMART_ROLLUP_ADDR: &str = "127.0.0.1";
const SANDBOX_OCTEZ_NODE_ADDR: &str = "127.0.0.1";

fn octez_node_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_OCTEZ_NODE_ADDR, SANDBOX_OCTEZ_NODE_PORT
    )
}

fn octez_smart_rollup_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_OCTEZ_SMART_ROLLUP_ADDR, SANDBOX_OCTEZ_SMART_ROLLUP_PORT
    )
}

fn jstz_node_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_JSTZ_NODE_ADDR, SANDBOX_JSTZ_NODE_PORT
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

fn init_node(cfg: &Config) -> Result<()> {
    // 1. Initialize the octez-node configuration
    debug!("Initializing octez-node");

    cfg.octez_node()?.config_init(
        "sandbox",
        &format!("127.0.0.1:{}", SANDBOX_OCTEZ_NODE_PORT),
        &format!("127.0.0.1:{}", SANDBOX_OCTEZ_NODE_RPC_PORT),
        0,
    )?;
    debug!("\tInitialized octez-node configuration");

    // 2. Generate an identity
    debug!("Generating identity...");
    cfg.octez_node()?.generate_identity()?;
    debug!("done");
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
    cfg.octez_client()?.wait_for_node_to_bootstrap()?;
    debug!("Node bootstrapped");

    // 3. Import activator and bootstrap accounts
    debug!("Importing activator account");
    cfg.octez_client()?
        .import_secret_key(ACTIVATOR_ACCOUNT_ALIAS, ACTIVATOR_ACCOUNT_SK)?;

    // 4. Activate alpha
    debug!("Activating alpha...");
    cfg.octez_client()?.activate_protocol(
        "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        "1",
        ACTIVATOR_ACCOUNT_ALIAS,
        SANDBOX_PARAMS_PATH,
    )?;
    debug!("Protocol activated");

    // 5. Import bootstrap accounts
    for (i, bootstrap_account) in SANDBOX_BOOTSTRAP_ACCOUNTS.iter().enumerate() {
        let name = format!("bootstrap{}", i + 1);
        cfg.octez_client()?
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
        .octez_client()?
        .bake(log_file, &["for", "--minimal-timestamp"]);
    Ok(())
}

async fn run_jstz_node() -> Result<()> {
    let local = task::LocalSet::new();

    local
        .run_until(async {
            task::spawn_local(async {
                debug!("Jstz node started 🎉");

                jstz_node::run(
                    SANDBOX_JSTZ_NODE_ADDR,
                    SANDBOX_JSTZ_NODE_PORT,
                    &format!(
                        "http://{}:{}",
                        SANDBOX_OCTEZ_SMART_ROLLUP_ADDR, SANDBOX_OCTEZ_SMART_ROLLUP_PORT
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
    init_node(cfg)?;

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
        &cfg.octez_client()?,
        OPERATOR_ADDRESS,
        ctez_bootstrap_accounts(),
    )?;

    let bridge =
        BridgeContract::deploy(&cfg.octez_client()?, OPERATOR_ADDRESS, &ctez_address)?;

    debug!("Bridge deployed at {}", bridge);

    // 6. Create an installer kernel
    debug!("Creating installer kernel...");

    let preimages_dir = TempDir::with_prefix("jstz_sandbox_preimages")?.into_path();

    let installer = make_installer(Path::new(JSTZ_KERNEL_PATH), &preimages_dir, &bridge)?;
    debug!(
        "Installer kernel created with preimages at {:?}",
        preimages_dir
    );

    // 7. Originate the rollup
    let rollup = JstzRollup::deploy(&cfg.octez_client()?, OPERATOR_ADDRESS, &installer)?;

    debug!("`jstz_rollup` originated at {}", rollup);

    // 8. As a thread, start rollup node
    debug!("Starting rollup node...");

    let logs_dir = logs_dir()?;
    let rollup_node = OctezThread::from_child(rollup.run(
        &cfg.octez_rollup_node()?,
        OPERATOR_ADDRESS,
        &preimages_dir,
        &logs_dir,
        "127.0.0.1",
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    )?);
    debug!("Started octez-smart-rollup-node");

    // 9. Set the rollup address in the bridge
    bridge.set_rollup(&cfg.octez_client()?, OPERATOR_ADDRESS, &rollup)?;
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

    // Create logs directory
    fs::create_dir_all(logs_dir()?)?;

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
