use std::{
    fs::File,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use anyhow::Result;

use crate::config::Config;

fn output(command: &mut Command) -> Result<String> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Command {:?} failed:\n {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub struct OctezClient;

impl OctezClient {
    fn command(cfg: &Config) -> Result<Command> {
        let octez_client_dir = &cfg.sandbox()?.octez_client_dir;

        let mut command = Command::new(cfg.octez_path.join("octez-client"));
        command.args([
            "-base-dir",
            octez_client_dir.to_str().expect("Invalid path"),
            "-endpoint",
            &format!("http://127.0.0.1:{}", cfg.octez_node_rpc_port),
        ]);

        command.env("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");

        Ok(command)
    }

    pub fn originate_contract(
        cfg: &Config,
        name: &str,
        source: &str,
        script: &str,
        storage: &str,
    ) -> Result<String> {
        let output = output(Self::command(cfg)?.args([
            "originate",
            "contract",
            name,
            "transferring",
            "0",
            "from",
            source,
            "running",
            script,
            "--init",
            storage,
            "--burn-cap",
            "999",
            "--force",
        ]))?;

        // Extract address from origination
        // TODO: Replace with KT1 regex
        let address = output
            .lines()
            .find(|line| line.contains("New contract"))
            .unwrap()
            .split_whitespace()
            .nth(2)
            .unwrap()
            .to_string();

        Ok(address)
    }

    pub fn transfer(
        cfg: &Config,
        from: &str,
        to: &str,
        entrypoint: &str,
        parameter: &str,
    ) -> Result<()> {
        output(Self::command(cfg)?.args([
            "transfer",
            "0",
            "from",
            from,
            "to",
            to,
            "--entrypoint",
            entrypoint,
            "--arg",
            parameter,
            "--burn-cap",
            "999",
        ]))?;
        Ok(())
    }

    pub fn send_rollup_external_message<T: AsRef<[u8]>>(
        cfg: &Config,
        source: &str,
        message: T,
    ) -> Result<()> {
        output(Self::command(cfg)?.args([
            "send",
            "smart",
            "rollup",
            "message",
            &format!("hex:[ \"{}\" ]", hex::encode(message)),
            "from",
            source,
        ]))?;
        Ok(())
    }

    pub fn rpc(cfg: &Config, options: &[&str]) -> Result<String> {
        output(Self::command(cfg)?.arg("rpc").args(options))
    }

    pub fn wait_for_node_to_bootstrap(cfg: &Config) -> Result<()> {
        output(Self::command(cfg)?.arg("bootstrapped"))?;
        Ok(())
    }

    pub fn bake(cfg: &Config, log_file: &File, options: &[&str]) -> Result<String> {
        output(
            Self::command(cfg)?
                .arg("bake")
                .stdout(Stdio::from(log_file.try_clone()?))
                .stderr(Stdio::from(log_file.try_clone()?))
                .args(options),
        )
    }

    pub fn import_secret_key(cfg: &Config, name: &str, sk: &str) -> Result<()> {
        output(Self::command(cfg)?.args(["import", "secret", "key", name, sk]))?;
        Ok(())
    }

    pub fn activate_protocol(
        cfg: &Config,
        protocol: &str,
        fitness: &str,
        key: &str,
        parameters_file: &str,
    ) -> Result<()> {
        output(Self::command(cfg)?.args([
            "-block",
            "genesis",
            "activate",
            "protocol",
            protocol,
            "with",
            "fitness",
            fitness,
            "and",
            "key",
            key,
            "and",
            "parameters",
            parameters_file,
        ]))?;
        Ok(())
    }

    pub fn originate_rollup(
        cfg: &Config,
        account: &str,
        name: &str,
        kind: &str,
        r#type: &str,
        kernel: &str,
    ) -> Result<String> {
        let output = output(Self::command(cfg)?.args([
            "originate",
            "smart",
            "rollup",
            name,
            "from",
            account,
            "of",
            "kind",
            kind,
            "of",
            "type",
            r#type,
            "with",
            "kernel",
            kernel,
            "--burn-cap",
            "999",
        ]))?;

        // TODO: Replace with sr1 regex
        let mut address: String = Default::default();
        for line in output.lines() {
            if line.contains("Address:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(value) = parts.iter().find(|&&word| word.starts_with("sr1")) {
                    address = value.to_string();
                }
            }
        }

        Ok(address)
    }
}

pub struct OctezNode;

impl OctezNode {
    fn command(cfg: &Config) -> Command {
        Command::new(cfg.octez_path.join("octez-node"))
    }

    pub fn config_init(cfg: &Config, network: &str, connections: &str) -> Result<()> {
        output(
            Self::command(cfg).args([
                "config",
                "init",
                "--network",
                network,
                "--data-dir",
                cfg.sandbox()?
                    .octez_node_dir
                    .to_str()
                    .expect("Invalid path"),
                "--net-addr",
                &format!("127.0.0.1:{}", cfg.octez_node_port),
                "--rpc-addr",
                &format!("127.0.0.1:{}", cfg.octez_node_rpc_port),
                "--connections",
                connections,
            ]),
        )?;
        Ok(())
    }

    pub fn generate_identity(cfg: &Config) -> Result<()> {
        output(
            Self::command(cfg).args([
                "identity",
                "generate",
                "--data-dir",
                cfg.sandbox()?
                    .octez_node_dir
                    .to_str()
                    .expect("Invalid path"),
            ]),
        )?;
        Ok(())
    }

    pub fn run(cfg: &Config, log_file: &PathBuf, options: &[&str]) -> Result<Child> {
        let log_file = File::create(log_file)?;

        Ok(Self::command(cfg)
            .args([
                "run",
                "--data-dir",
                cfg.sandbox()?
                    .octez_node_dir
                    .to_str()
                    .expect("Invalid path"),
            ])
            .args(options)
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?))
            .spawn()?)
    }
}

pub struct OctezRollupNode;

impl OctezRollupNode {
    fn command(cfg: &Config) -> Result<Command> {
        let octez_client_dir = &cfg.sandbox()?.octez_client_dir;

        let mut command = Command::new(cfg.octez_path.join("octez-smart-rollup-node"));
        command.args([
            "-base-dir",
            octez_client_dir.to_str().expect("Invalid path"),
            "-endpoint",
            &format!("http://127.0.0.1:{}", cfg.octez_node_rpc_port),
        ]);

        Ok(command)
    }

    pub fn run(
        cfg: &Config,
        log_file: &PathBuf,
        rollup: &str,
        operator: &str,
        options: &[&str],
    ) -> Result<Child> {
        let log_file = File::create(log_file)?;

        Ok(Self::command(cfg)?
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?))
            .args([
                "run",
                "operator",
                "for",
                rollup,
                "with",
                "operators",
                operator,
                "--data-dir",
                cfg.sandbox()?
                    .octez_rollup_node_dir
                    .to_str()
                    .expect("Invalid path"),
            ])
            .args(options)
            .spawn()?)
    }
}
