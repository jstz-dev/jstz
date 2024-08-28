use futures::Future;
use indicatif::{ProgressBar, ProgressStyle};
use jstz_rollup::{rollup::make_installer, Exchanger, JstzRollup, NativeBridge};
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use octez::OctezThread;
use regex::Regex;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use std::io::Write;
use std::{
    cell::RefCell,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Seek},
    path::PathBuf,
    process::{Child, Command, Stdio},
    rc::Rc,
    thread::{self, sleep},
    time::Duration,
};

use console::style;
use in_container::in_container;
use log::info;
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};
use tempfile::TempDir;
use tokio::task::{self, JoinHandle};

macro_rules! debug {
    ($file:expr, $($arg:tt)*) => {
        writeln!($file, $($arg)*).expect("Failed to write to log file");
        $file.flush().expect("Failed to flush log file");
    };
}

use crate::{
    config::{jstz_home_dir, Config, SandboxConfig},
    error::{anyhow, bail_user_error, Result},
    sandbox::{
        SANDBOX_BOOTSTRAP_ACCOUNTS, SANDBOX_JSTZ_NODE_PORT, SANDBOX_LOCAL_HOST_ADDR,
        SANDBOX_OCTEZ_NODE_PORT, SANDBOX_OCTEZ_NODE_RPC_PORT,
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    },
    term::{self, styles},
};

use super::SANDBOX_BOOTSTRAP_ACCOUNT_XTZ_AMOUNT;

fn octez_node_endpoint() -> String {
    format!(
        "http://{}:{}",
        SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_RPC_PORT
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

fn node_log_path(cfg: &Config) -> PathBuf {
    cfg.sandbox_logs_dir().join("node.log")
}

fn client_log_path(cfg: &Config) -> PathBuf {
    cfg.sandbox_logs_dir().join("client.log")
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

pub struct SandboxBootstrapAccount<'a> {
    pub address: &'a str,
    pub secret: &'a str,
}

pub struct Sandbox {
    config: Rc<RefCell<Config>>,
    log_file: Rc<RefCell<File>>,
    octez_node: Option<OctezThread>,
    octez_baker: Option<OctezThread>,
    rollup_node: Option<OctezThread>,
    jstz_node: Option<JoinHandle<Result<()>>>,
    task_set: task::LocalSet,
}

impl Sandbox {
    pub fn new(config: Rc<RefCell<Config>>, log_file: Rc<RefCell<File>>) -> Sandbox {
        Sandbox {
            config,
            log_file,
            octez_node: None,
            octez_baker: None,
            rollup_node: None,
            jstz_node: None,
            task_set: task::LocalSet::new(),
        }
    }

    pub fn shutdown(&mut self) -> Result<()> {
        if let Some(mut t) = self.rollup_node.take() {
            t.shutdown()?;
        }
        if let Some(mut b) = self.octez_baker.take() {
            b.shutdown()?;
        }
        if let Some(mut n) = self.octez_node.take() {
            n.shutdown()?;
        }
        if let Some(j) = self.jstz_node.take() {
            j.abort();
        }

        self.remove_sandbox_from_config()?;
        Ok(())
    }

    pub async fn join(mut self) -> Result<()> {
        let mut threads: Vec<OctezThread> = Vec::new();
        let mut jstz_node = Option::None;
        if let Some(n) = self.octez_node.take() {
            threads.push(n);
        }

        if let Some(jn) = self.jstz_node.take() {
            jstz_node = Some(jn);
        }

        if let Some(b) = self.octez_baker.take() {
            threads.push(b)
        }

        if let Some(r) = self.rollup_node.take() {
            threads.push(r)
        }

        let mut signals = Signals::new([SIGINT, SIGTERM])?;

        // Loop to notify when either the octez binaries or jstz node is not running
        // or if SIGINT/SIGTERM was received. This ensures that all processes are
        // shutdown when any process is shutdown.
        self.task_set
            .run_until(async move {
                task::spawn_local(async move {
                    'main_loop: loop {
                        for thread in threads.iter() {
                            if !thread.is_running() {
                                info!(
                                    "Octez or Baker node is not running . Shutting down..",
                                );
                                break 'main_loop;
                            }
                        }

                        if let Some(jn) = &jstz_node {
                            if jn.is_finished() {
                                info!("Jstz node not running. Shutting down..",);
                                break 'main_loop;
                            }
                        }

                        for signal in signals.pending() {
                            match signal {
                                SIGINT | SIGTERM => {
                                    info!(
                                        "Received signal {:?}, shutting down...",
                                        signal
                                    );
                                    break 'main_loop;
                                }
                                _ => unreachable!(),
                            }
                        }

                        // yield point
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }

                    for mut thread in threads {
                        let _ = thread.shutdown();
                    }

                    if let Some(jn) = &jstz_node {
                        if !jn.is_finished() {
                            jn.abort()
                        }
                    }
                })
                .await
            })
            .await?;

        self.remove_sandbox_from_config()?;

        Ok(())
    }

    fn remove_sandbox_from_config(&mut self) -> Result<()> {
        let mut config = self.config.borrow_mut();
        config.reload()?;
        config.sandbox = None;
        config.save()
    }

    pub fn set_octez_node(&mut self, octez_node: OctezThread) -> Result<()> {
        match self.octez_node {
            Some(_) => {
                debug!(self.log_file.borrow_mut(), "Error: Octez node already set");
                panic!();
            }
            None => {
                self.octez_node = Some(octez_node);
                Ok(())
            }
        }
    }

    pub fn set_baker(&mut self, octez_baker: OctezThread) -> Result<()> {
        match self.octez_baker {
            Some(_) => {
                debug!(self.log_file.borrow_mut(), "Error: Octez baker already set");
                panic!();
            }
            None => {
                self.octez_baker = Some(octez_baker);
                Ok(())
            }
        }
    }

    pub fn set_rollup_node(&mut self, rollup_node: OctezThread) -> Result<()> {
        match self.rollup_node {
            Some(_) => {
                debug!(self.log_file.borrow_mut(), "Error: Rollup node already set");
                panic!();
            }
            None => {
                self.rollup_node = Some(rollup_node);
                Ok(())
            }
        }
    }

    pub fn set_jstz_node(
        &mut self,
        jstz_node: impl Future<Output = Result<()>> + 'static,
    ) -> Result<()> {
        match self.jstz_node {
            Some(_) => {
                debug!(self.log_file.borrow_mut(), "Error: jstz node already set");
                panic!();
            }
            None => {
                // Run the jstz node in task LocalSet
                let jstz_node = self.task_set.spawn_local(jstz_node);
                self.jstz_node = Some(jstz_node);
                Ok(())
            }
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            debug!(
                self.log_file.borrow_mut(),
                "Failed to shutdown sandbox: {:?}", e
            );
        }
    }
}

const ACTIVATOR_ACCOUNT_ALIAS: &str = "activator";
fn sandbox_daemon_log_path(cfg: &Config) -> PathBuf {
    cfg.sandbox_logs_dir().join("sandbox_daemon.log")
}

const ACTIVATOR_ACCOUNT_SK: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";

const OPERATOR_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"; // bootstrap1

fn cached_identity_path() -> PathBuf {
    jstz_home_dir().join("octez-node-identity.json")
}

fn octez_node_identity_path(cfg: &Config) -> Result<PathBuf> {
    Ok(cfg.octez_node()?.octez_node_dir.join("identity.json"))
}

fn generate_identity(log_file: &mut File, cfg: &Config) -> Result<()> {
    let cached_identity_path = cached_identity_path();
    let octez_node_identity_path = octez_node_identity_path(cfg)?;

    if cached_identity_path.exists() {
        debug!(log_file, "Cached identity hit");
        fs::copy(cached_identity_path, octez_node_identity_path)?;
        return Ok(());
    }

    debug!(log_file, "Cached identity miss");
    debug!(log_file, "Generating identity...");
    cfg.octez_node()?.generate_identity()?;
    debug!(log_file, "Identity generated");

    fs::copy(octez_node_identity_path, cached_identity_path)?;
    debug!(log_file, "Cached identity");

    Ok(())
}

// Number of sandbox steps - calls to `progress_step` - to complete
const MAX_PROGRESS: u32 = 16;
fn progress_step(log_file: &mut File, progress: &mut u32) {
    *progress += 1;
    debug!(log_file, "({})", progress);
}

fn init_node(log_file: &mut File, progress: &mut u32, cfg: &Config) -> Result<()> {
    // 1. Initialize the octez-node configuration
    debug!(log_file, "Initializing octez-node");

    cfg.octez_node()?.config_init(
        "sandbox",
        &format!("{}:{}", SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_PORT),
        &format!(
            "{}:{}",
            SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_RPC_PORT
        ),
        0,
    )?;
    debug!(log_file, "\tInitialized octez-node configuration");

    // 2. Generate an identity
    progress_step(log_file, progress);
    generate_identity(log_file, cfg)?;

    Ok(())
}

fn start_node(cfg: &Config) -> Result<Child> {
    // Run the octez-node in sandbox mode
    let log_file = File::create(node_log_path(cfg))?;

    cfg.octez_node()?.run(
        &log_file,
        &[
            "--synchronisation-threshold",
            "0",
            "--network",
            "sandbox",
            "--sandbox",
            sandbox_path().to_str().expect("Invalid path"),
            "--history-mode",
            "archive",
        ],
    )
}

fn is_node_running(cfg: &Config) -> Result<bool> {
    Ok(cfg
        .octez_client_sandbox()?
        .rpc(&["get", "/chains/main/blocks/head/hash"])
        .is_ok())
}

fn wait_for_node_to_initialize(log_file: &mut File, cfg: &Config) -> Result<()> {
    if is_node_running(cfg)? {
        return Ok(());
    }

    debug!(log_file, "Waiting for node to initialize...");
    while !is_node_running(cfg)? {
        sleep(Duration::from_secs(1));
    }

    debug!(log_file, "Node initialized");
    Ok(())
}

fn init_client(log_file: &mut File, progress: &mut u32, cfg: &Config) -> Result<()> {
    // 1. Wait for the node to initialize
    wait_for_node_to_initialize(log_file, cfg)?;

    // 2. Wait for the node to be bootstrapped
    progress_step(log_file, progress);
    debug!(log_file, "Waiting for node to bootstrap...");
    cfg.octez_client_sandbox()?.wait_for_node_to_bootstrap()?;
    debug!(log_file, " done");

    // 3. Import activator and bootstrap accounts
    progress_step(log_file, progress);
    debug!(log_file, "Importing activator account...");
    cfg.octez_client_sandbox()?
        .import_secret_key(ACTIVATOR_ACCOUNT_ALIAS, ACTIVATOR_ACCOUNT_SK)?;
    debug!(log_file, "done");

    // 4. Activate alpha
    progress_step(log_file, progress);
    debug!(log_file, "Activating alpha...");
    cfg.octez_client_sandbox()?.activate_protocol(
        "PtParisBxoLz5gzMmn3d9WBQNoPSZakgnkMC2VNuQ3KXfUtUQeZ",
        "1",
        "activator",
        sandbox_params_path().to_str().expect("Invalid path"),
    )?;
    debug!(log_file, " done");

    // 5. Import bootstrap accounts
    progress_step(log_file, progress);
    for (i, bootstrap_account) in SANDBOX_BOOTSTRAP_ACCOUNTS.iter().enumerate() {
        let name = format!("bootstrap{}", i + 1);
        cfg.octez_client_sandbox()?
            .import_secret_key(&name, bootstrap_account.secret)?;
        debug!(
            log_file,
            "Imported account {}. address: {}, secret: {}",
            name,
            bootstrap_account.address,
            bootstrap_account.secret
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
fn spawn_jstz_node(cfg: &Config) -> Result<impl Future<Output = Result<()>> + 'static> {
    let log_path = sandbox_daemon_log_path(cfg);
    let kernel_log_path = cfg.sandbox_logs_dir().join("kernel.log");
    let mut log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_path.clone())?;
    debug!(log_file, "Jstz node started 🎉");

    Ok(jstz_node::run(
        SANDBOX_LOCAL_HOST_ADDR,
        SANDBOX_JSTZ_NODE_PORT,
        format!(
            "http://{}:{}",
            SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_SMART_ROLLUP_PORT
        ),
        kernel_log_path,
    ))
}

fn start_sandbox(
    log_file: &mut File,
    progress: &mut u32,
    cfg: &mut Config,
) -> Result<Sandbox> {
    let config = Rc::new(RefCell::new(cfg.clone()));
    let logs = Rc::new(RefCell::new(log_file.try_clone()?));
    let mut sandbox = Sandbox::new(config, logs);

    // 1. Init node
    init_node(log_file, progress, cfg)?;

    // 2. As a thread, start node
    progress_step(log_file, progress);
    let node = OctezThread::from_child(start_node(cfg)?);
    sandbox.set_octez_node(node)?;
    debug!(log_file, "Started octez-node");

    // 3. Init client
    progress_step(log_file, progress);
    init_client(log_file, progress, cfg)?;
    debug!(log_file, "Initialized octez-client");

    // 4. As a thread, start baking
    progress_step(log_file, progress);
    let client_logs = File::create(client_log_path(cfg))?;
    let baker = OctezThread::new(cfg.clone(), move |cfg| {
        client_bake(cfg, &client_logs)?;
        Ok(())
    });
    sandbox.set_baker(baker)?;
    debug!(log_file, "Started baker (using octez-client)");

    // 4.1 Deploy the XTZ ticket exchanger
    let client = cfg.octez_client_sandbox()?;
    let exchanger = Exchanger::deploy(&client, OPERATOR_ADDRESS)?;
    debug!(log_file, "Exchanger deployed at {}", exchanger);

    // 5. Create an installer kernel
    progress_step(log_file, progress);
    debug!(log_file, "Creating installer kernel...");

    let preimages_dir = TempDir::with_prefix("jstz_sandbox_preimages")?.into_path();

    let installer = make_installer(&jstz_kernel_path(), &preimages_dir, &exchanger)?;
    debug!(
        log_file,
        "Installer kernel created with preimages at {:?}", preimages_dir
    );

    // 6. Originate the rollup
    progress_step(log_file, progress);
    let rollup =
        JstzRollup::deploy(&cfg.octez_client_sandbox()?, OPERATOR_ADDRESS, &installer)?;
    debug!(log_file, "`jstz_rollup` originated at {}", rollup);

    // 7. As a thread, start rollup node
    progress_step(log_file, progress);
    debug!(log_file, "Starting rollup node...");

    let logs_dir = cfg.sandbox_logs_dir();
    let rollup_node = OctezThread::from_child(rollup.run(
        &cfg.octez_rollup_node_sandbox()?,
        OPERATOR_ADDRESS,
        &preimages_dir,
        &logs_dir,
        SANDBOX_LOCAL_HOST_ADDR,
        SANDBOX_OCTEZ_SMART_ROLLUP_PORT,
    )?);
    sandbox.set_rollup_node(rollup_node)?;
    debug!(log_file, "Started octez-smart-rollup-node");

    // 8. Deploy bridge
    progress_step(log_file, progress);
    debug!(log_file, "Deploying bridge...");

    progress_step(log_file, progress);

    let bridge = NativeBridge::deploy(&client, OPERATOR_ADDRESS, &exchanger, &rollup)?;

    debug!(log_file, "Bridge deployed at {}", bridge);

    // 9. Set the rollup address in the bridge
    progress_step(log_file, progress);
    debug!(log_file, "`jstz_bridge` `rollup` address set to {}", rollup);

    Ok(sandbox)
}

fn format_sandbox_bootstrap_accounts() -> Table {
    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Address"),
        Cell::new("XTZ Balance"),
    ]));

    for (i, bootstrap_account) in SANDBOX_BOOTSTRAP_ACCOUNTS.iter().enumerate() {
        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "(bootstrap{}) {}",
                i + 1,
                bootstrap_account.address
            )),
            Cell::new(&SANDBOX_BOOTSTRAP_ACCOUNT_XTZ_AMOUNT.to_string()),
        ]));
    }

    table
}

pub async fn run_sandbox(cfg: &mut Config) -> Result<()> {
    // Create logs directory
    fs::create_dir_all(cfg.sandbox_logs_dir())?;

    let log_path = sandbox_daemon_log_path(cfg);
    let mut log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path.clone())?;

    let mut progress = 0;

    // 1. Configure sandbox
    debug!(log_file, "Configuring sandbox...");
    // TODO: Temp directories should be deleted
    let sandbox_cfg = SandboxConfig {
        pid: std::process::id(),
        octez_client_dir: TempDir::with_prefix("octez_client")?.into_path(), // into_path() causes temp directories to be persisted!
        octez_node_dir: TempDir::with_prefix("octez_node")?.into_path(),
        octez_rollup_node_dir: TempDir::with_prefix("octez_rollup_node")?.into_path(),
    };

    cfg.sandbox = Some(sandbox_cfg);
    debug!(log_file, "Sandbox configured {:?}", cfg.sandbox);

    // 2. Start sandbox
    progress_step(&mut log_file, &mut progress);
    let mut sandbox = start_sandbox(&mut log_file, &mut progress, cfg)?;
    debug!(log_file, "Sandbox started 🎉");

    // 3. Save config
    progress_step(&mut log_file, &mut progress);
    debug!(log_file, "Saving sandbox config");
    cfg.save()?;
    // 4. Wait for the sandbox or jstz-node to shutdown (either by the user or by an error)
    let jstz = spawn_jstz_node(cfg)?;
    sandbox.set_jstz_node(jstz)?;
    sandbox.join().await?;
    Ok(())
}

fn print_banner() {
    info!("{}", style(SANDBOX_BANNER).bold());
    info!(
        "        {} {}",
        env!("CARGO_PKG_VERSION"),
        styles::url(env!("CARGO_PKG_REPOSITORY"))
    );
    info!("");
}

fn start_background_process(cfg: &Config) -> Result<Child> {
    let path = sandbox_daemon_log_path(cfg);
    let stdout_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path.clone())?;
    let child = Command::new(std::env::current_exe()?)
        .args(["sandbox", "start", "--background"])
        .stdout(Stdio::from(stdout_file))
        .spawn()?;

    Ok(child)
}

fn run_progress_bar(cfg: &Config, mut child: Option<Child>) -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&sandbox_daemon_log_path(cfg))?;
    let mut reader = BufReader::new(file);
    let mut buffer = String::new();

    let regex = Regex::new(r"\((\d+)\)")?;

    let mut progress: u32 = 0;

    let progress_bar = ProgressBar::new(MAX_PROGRESS as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}")?,
    );

    loop {
        reader.stream_position()?;

        while reader.read_line(&mut buffer)? > 0 {
            if let Some(captures) = regex.captures(&buffer) {
                if let Some(matched) = captures.get(1) {
                    if let Ok(num) = matched.as_str().parse::<u32>() {
                        progress = num;
                    }
                }
            }
            buffer.clear();
        }

        if progress == MAX_PROGRESS {
            progress_bar.finish_and_clear();
            break;
        }

        if let Some(child) = child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                progress_bar.finish_and_clear();

                bail_user_error!("Sandbox failed to start: {:}", status);
            }
        }

        progress_bar.set_position(progress.into());
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

fn print_sandbox_info() {
    // Print sandbox info
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
}

fn wait_for_termination(pid: Pid) -> Result<()> {
    loop {
        let result: nix::Result<()> = kill(pid, Signal::SIGTERM);
        match result {
            // Sending 0 as the signal just checks for the process existence
            Ok(_) => {
                // Process exists, continue waiting
                thread::sleep(Duration::from_millis(100));
            }
            Err(nix::Error::ESRCH) => {
                // No such process, it has terminated
                break;
            }
            Err(e) => {
                // An unexpected error occurred
                bail_user_error!("Failed to kill the sandbox process: {:?}", e)
            }
        }
    }
    Ok(())
}

pub fn stop_sandbox(restart: bool) -> Result<()> {
    if in_container() {
        bail_user_error!("Stopping the sandbox is not supported in this environment. Please run CTRL+C to stop the sandbox.");
    }

    let cfg = Config::load()?;

    match cfg.sandbox {
        Some(sandbox_cfg) => {
            if !restart {
                info!("Stopping the sandbox...");
            }
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;

            wait_for_termination(pid)?;

            Ok(())
        }
        None => {
            if !restart {
                bail_user_error!("The sandbox is not running!")
            } else {
                Ok(())
            }
        }
    }
}

pub async fn main(detach: bool, background: bool, cfg: &mut Config) -> Result<()> {
    if background {
        run_sandbox(cfg).await?;
        return Ok(());
    }

    if cfg.sandbox.is_some() {
        bail_user_error!("The sandbox is already running!");
    }

    if detach {
        if in_container() {
            bail_user_error!("Detaching from the terminal is not supported in this environment. Please run `jstz sandbox start` without the `--detach` flag.");
        }

        let child = start_background_process(cfg)?;
        run_progress_bar(cfg, Some(child))?;

        // Reload the config to get the pid of the sandbox
        cfg.reload()?;
        info!(
            "Sandbox pid: {}.   Use `{}` to stop the sandbox background process.",
            cfg.sandbox()?.pid,
            term::styles::command("jstz sandbox stop").bold()
        );
    } else {
        // TODO: update using mpsc style or shared with mutex
        let handle = {
            // Clone config to move into the thread
            let cfg = cfg.clone();
            thread::spawn(move || -> Result<()> {
                print_banner();

                run_progress_bar(&cfg, None)?;

                print_sandbox_info();

                Ok(())
            })
        };

        run_sandbox(cfg).await?;

        handle
            .join()
            .map_err(|_| anyhow!("Failed to join sandbox progress bar thread"))??;
    }
    Ok(())
}
