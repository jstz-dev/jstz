use std::{
    fmt::{self, Display},
    fs::{self, File},
    path::Path,
    process::Child,
};

use anyhow::Result;
use derive_more::{Deref, DerefMut};
use fs_extra::dir::CopyOptions;
use octez::{OctezClient, OctezRollupNode};
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tezos_smart_rollup_host::path::{OwnedPath, RefPath};
use tezos_smart_rollup_installer::{
    installer, preimages, KERNEL_BOOT_PATH, PREPARE_KERNEL_PATH,
};
use tezos_smart_rollup_installer_config::binary::owned::{
    OwnedBytes, OwnedConfigInstruction, OwnedConfigProgram,
};

use crate::BridgeContract;

const TICKETER_PATH: RefPath = RefPath::assert_from(b"/ticketer");
const ROLLUP_MICHELSON_TYPE: &str = "(pair bytes (ticket (pair nat (option bytes))))";

pub fn make_installer(
    kernel_file: &Path,
    preimages_dir: &Path,
    bridge_contract: &BridgeContract,
) -> Result<Vec<u8>> {
    let root_hash = preimages::content_to_preimages(kernel_file, preimages_dir)?;

    let installer_program = OwnedConfigProgram(vec![
        // 1. Prepare kernel installer
        OwnedConfigInstruction::reveal_instr(
            root_hash,
            OwnedPath::from(PREPARE_KERNEL_PATH),
        ),
        OwnedConfigInstruction::move_instr(
            OwnedPath::from(PREPARE_KERNEL_PATH),
            OwnedPath::from(KERNEL_BOOT_PATH),
        ),
        // 2. Set `jstz` ticketer as the bridge contract address
        OwnedConfigInstruction::set_instr(
            OwnedBytes(bincode::serialize(&ContractKt1Hash::from_base58_check(
                bridge_contract,
            )?)?),
            OwnedPath::from(TICKETER_PATH),
        ),
    ]);

    let installer = installer::with_config_program(installer_program);

    Ok(installer)
}

#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct JstzRollup(String);

impl Display for JstzRollup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<SmartRollupHash> for JstzRollup {
    fn from(hash: SmartRollupHash) -> Self {
        Self(hash.to_base58_check())
    }
}

impl JstzRollup {
    pub fn deploy(
        client: &OctezClient,
        operator: &str,
        installer: &[u8],
    ) -> Result<Self> {
        let address = client.originate_rollup(
            operator,
            "jstz_rollup",
            "wasm_2_0_0",
            ROLLUP_MICHELSON_TYPE,
            &hex::encode(installer),
        )?;

        Ok(Self(address))
    }

    pub fn run(
        &self,
        rollup_node: &OctezRollupNode,
        operator: &str,
        preimages_dir: &Path,
        logs_dir: &Path,
        addr: &str,
        port: u16,
    ) -> Result<Child> {
        let rollup_log_file = File::create(logs_dir.join("rollup.log"))?;

        // 1. Copy kernel installer preimages to rollup node directory
        let rollup_node_preimages_dir =
            rollup_node.octez_rollup_node_dir.join("wasm_2_0_0");

        fs::create_dir_all(&rollup_node_preimages_dir)?;
        fs_extra::dir::copy(
            preimages_dir,
            &rollup_node_preimages_dir,
            &CopyOptions {
                content_only: true,
                ..Default::default()
            },
        )?;

        // 2. Run the rollup node (configuring the kernel log file)
        let kernel_log = logs_dir.join("kernel.log");
        // ensure the log exists
        let _ = fs::File::options()
            .append(true)
            .create(true)
            .open(&kernel_log)?;

        rollup_node.run(
            addr,
            port,
            &rollup_log_file,
            &self.0,
            operator,
            &[
                "--log-kernel-debug",
                "--log-kernel-debug-file",
                kernel_log.to_str().expect("Invalid path"),
            ],
        )
    }
}
