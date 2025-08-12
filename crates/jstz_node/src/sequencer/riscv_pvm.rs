use octez_riscv::{
    machine_state::block_cache::{block, DefaultCacheConfig},
    program::Program,
    pvm::{hooks::PvmHooks, Pvm, PvmStatus},
    state_backend::owned_backend::Owned,
    stepper::{pvm::reveals::RevealRequestResponseMap, StepperStatus},
};
use tezos_crypto_rs::hash::SmartRollupHash;

use std::{
    io::{stdout, Write},
    ops::Bound,
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, Arc},
};

use crate::sequencer::worker::write_heartbeat;

type MemoryConfig = octez_riscv::machine_state::memory::M32G;
type BlockImplInner = block::Jitted<block::OutlineCompiler<MemoryConfig>, MemoryConfig>;

pub struct JstzPvm {
    pvm: Pvm<MemoryConfig, DefaultCacheConfig, BlockImplInner, Owned>,
    hooks: DebugLogHook,
    _origination_level: u32,
    reveal_request_response_map: RevealRequestResponseMap,
    heartbeat: Arc<AtomicU64>,
}

struct DebugLogHook {
    file: Box<dyn Write>,
}

impl PvmHooks for DebugLogHook {
    fn write_debug_bytes(&mut self, bytes: &[u8]) {
        let _ = self.file.write_all(bytes);
    }
}

impl JstzPvm {
    /// Create a new PVM stepper.
    pub fn new(
        kernel_path: PathBuf,
        rollup_address: &SmartRollupHash,
        origination_level: u32,
        preimages_dir: Option<Box<Path>>,
        heartbeat: Arc<AtomicU64>,
        debug_log_path: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut pvm = Pvm::new(Default::default());

        let program = std::fs::read(kernel_path)?;
        let program = Program::<MemoryConfig>::from_elf(&program)?;

        pvm.setup_linux_process(&program)?;

        let reveal_request_response_map = RevealRequestResponseMap::new(
            rollup_address.as_ref().try_into().unwrap(),
            origination_level,
            preimages_dir,
        );

        let hooks = DebugLogHook {
            file: match debug_log_path {
                Some(v) => Box::new(std::fs::File::options().append(true).open(v)?),
                None => Box::new(stdout()),
            },
        };

        Ok(Self {
            pvm,
            hooks,
            _origination_level: origination_level,
            reveal_request_response_map,
            heartbeat,
        })
    }

    /// Non-continuing variant of [`Stepper::step_max`]
    pub fn step_max_once(&mut self, steps: Bound<usize>) -> StepperStatus {
        match self.pvm.status() {
            PvmStatus::Evaluating => {
                let steps = self.pvm.eval_max(&mut self.hooks, steps);
                StepperStatus::Running { steps }
            }

            PvmStatus::WaitingForInput => StepperStatus::Exited {
                steps: 0,
                success: true,
                status: "No new message".to_owned(),
            },

            PvmStatus::WaitingForReveal => {
                let reveal_request = self.pvm.reveal_request();

                let Some(reveal_response) = self
                    .reveal_request_response_map
                    .get_response(reveal_request.as_slice())
                else {
                    // TODO: RV-573: Handle incorrectly encoded request/ Unavailable data differently in the sandbox.
                    // When the PVM sends an incorrectly encoded reveal request, the stepper should return an error.
                    // When the PVM sends a request for unavailable data, the stepper should exit.
                    self.pvm.provide_reveal_error_response();

                    return StepperStatus::Running { steps: 1 };
                };

                let success = self.pvm.provide_reveal_response(&reveal_response);
                if success {
                    StepperStatus::Running { steps: 1 }
                } else {
                    StepperStatus::Errored {
                        steps: 0,
                        cause: "PVM was waiting for reveal response".to_owned(),
                        message: "Providing reveal response did not succeed".to_owned(),
                    }
                }
            }
        }
    }

    pub fn execute_operation(
        &mut self,
        encoded_operation: Vec<u8>,
        mut step_bounds: Bound<usize>,
    ) -> StepperStatus {
        let mut total_steps = 0usize;
        let mut container = Some(encoded_operation);

        loop {
            write_heartbeat(&self.heartbeat);
            let v = self.step_max_once(step_bounds);
            println!("{v:?}");
            match v {
                StepperStatus::Running { steps } => {
                    total_steps = total_steps.saturating_add(steps);
                    step_bounds = bound_saturating_sub(step_bounds, steps);
                }

                StepperStatus::Exited {
                    steps,
                    success,
                    status,
                } => {
                    total_steps = total_steps.saturating_add(steps);
                    match container.take() {
                        Some(payload) => {
                            let success = self.pvm.provide_inbox_message(0, 0, &payload);
                            if !success {
                                return StepperStatus::Errored {
                                    steps: 0,
                                    cause: "PVM was waiting for input".to_owned(),
                                    message: "Providing input did not succeed".to_owned(),
                                };
                            }
                        }
                        _ => {
                            break StepperStatus::Exited {
                                steps: total_steps,
                                success,
                                status,
                            };
                        }
                    }
                }

                StepperStatus::Errored {
                    steps,
                    cause,
                    message,
                } => {
                    break StepperStatus::Errored {
                        steps: total_steps.saturating_add(steps),
                        cause,
                        message,
                    };
                }
            }
        }
    }
}

fn bound_saturating_sub(bound: Bound<usize>, shift: usize) -> Bound<usize> {
    match bound {
        Bound::Included(x) => Bound::Included(x.saturating_sub(shift)),
        Bound::Excluded(x) => Bound::Excluded(x.saturating_sub(shift)),
        Bound::Unbounded => Bound::Unbounded,
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
    use jstz_kernel::inbox::encode_parsed_inbox_message;
    use tezos_crypto_rs::hash::SmartRollupHash;
    use tezos_smart_rollup::types::SmartRollupAddress;

    use crate::sequencer::tests::dummy_op;

    #[test]
    fn create_pvm() {
        let ticketer =
            SmartFunctionHash::from_base58("KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ")
                .expect("msg");
        let rollup_address =
            SmartRollupHash::from_base58_check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();

        let mut pvm = super::JstzPvm::new(
            PathBuf::from_str("/Users/huanchengchang/code/jstz/target/riscv64gc-unknown-linux-musl/release/kernel-executable").unwrap(),
            &rollup_address,
            0,
            Some(PathBuf::from_str("/tmp/t").unwrap().into_boxed_path()),
            Default::default(),
            Default::default()
        )
        .unwrap();

        let message = encode_parsed_inbox_message(
            &dummy_op(),
            &ticketer,
            &SmartRollupAddress::new(rollup_address),
        )
        .unwrap();
        println!(
            "final {:?}",
            pvm.execute_operation(message, std::ops::Bound::Unbounded)
        );
    }
}
