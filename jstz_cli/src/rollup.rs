use std::{io::Write, process};

use anyhow::Result;
use clap::Subcommand;
use jstz_core::kv::value::serialize;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup_installer_config::yaml::{Instr, SetArgs, YamlConfig};

use crate::config::Config;

pub(crate) fn build_installer(cfg: &Config, bridge_address: &str) -> Result<()> {
    //Convert address
    let bridge_address = ContractKt1Hash::from_base58_check(bridge_address)?;

    let instructions = YamlConfig {
        instructions: vec![Instr::Set(SetArgs {
            value: hex::encode(serialize(&bridge_address)),
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
    let mut installer_command = process::Command::new("smart-rollup-installer");

    installer_command.args(&[
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

#[derive(Subcommand)]
pub enum Command {
    BuildInstaller {
        /// The address of the bridge contract
        #[arg(value_name = "bridge-address")]
        bridge_address: String,
    },
}

pub fn exec(command: Command, cfg: &Config) -> Result<()> {
    match command {
        Command::BuildInstaller { bridge_address } => {
            build_installer(cfg, &bridge_address)
        }
    }
}
