use jstz_proto::operation::internal::InboxId;
use octez_riscv::machine_state::page_cache::Interpreted;
use octez_riscv::state_backend::owned_backend::Owned;
use octez_riscv::{
    program::Program,
    pvm::{hooks::PvmHooks, Pvm, PvmStatus},
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

pub struct JstzRiscvPvm {
    pvm: Pvm<MemoryConfig, Interpreted<MemoryConfig, Owned>, Owned>,
    hooks: DebugLogHook,
    reveal_request_response_map: RevealRequestResponseMap,
    heartbeat: Arc<AtomicU64>,
}

struct DebugLogHook {
    log_file: Box<dyn Write + Send>,
}

impl PvmHooks for DebugLogHook {
    fn write_debug_bytes(&mut self, bytes: &[u8]) {
        let _ = self.log_file.write_all(bytes);
    }
}

impl JstzRiscvPvm {
    /// Create a PVM that runs Jstz RISCV kernel.
    pub fn new(
        kernel_path: &Path,
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
            log_file: match debug_log_path {
                Some(v) => {
                    Box::new(std::fs::File::options().create(true).append(true).open(v)?)
                }
                None => Box::new(stdout()),
            },
        };

        Ok(Self {
            pvm,
            hooks,
            reveal_request_response_map,
            heartbeat,
        })
    }

    fn step_max_once(&mut self, steps: Bound<usize>) -> StepperStatus {
        match self.pvm.status() {
            PvmStatus::Evaluating => {
                let steps = self.pvm.eval_max(&mut self.hooks, steps);
                StepperStatus::Running { steps }
            }

            PvmStatus::WaitingForInput => StepperStatus::Exited {
                steps: 0,
                success: true,
                status: "Finished processing everything at hand".to_owned(),
            },

            PvmStatus::WaitingForReveal => {
                let reveal_request = self.pvm.reveal_request();

                let Some(reveal_response) = self
                    .reveal_request_response_map
                    .get_response(reveal_request.as_slice())
                else {
                    // TODO: Handle incorrectly encoded request/ Unavailable data differently in the sandbox.
                    // When the PVM sends an incorrectly encoded reveal request, the PVM wrapper should return an error.
                    // When the PVM sends a request for unavailable data, the PVM wrapper should exit.
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
        inbox_id: InboxId,
        encoded_operation: Vec<u8>,
        mut step_bounds: Bound<usize>,
    ) -> StepperStatus {
        let mut total_steps = 0usize;
        // `container` wraps around the actual operation and is supposed to be consumed only once.
        let mut container = Some(encoded_operation);

        // The loop keeps running step_max_once until the PVM is ready to process the message.
        // This is necessary because the PVM does other things from time to time, especially
        // at the very beginning of launching. We need to wait here until the PVM says it's
        // ready to handle messages. `StepperStatus::Exited` is the exact signal returned when
        // the PVM starts to idle. The message is then passed to the PVM and it will do its job.
        // When `StepperStatus::Exited` is returned again, we know that the PVM has finished
        // processing the message and we can wrap up this execution.
        loop {
            write_heartbeat(&self.heartbeat);
            match self.step_max_once(step_bounds) {
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
                            let success = self.pvm.provide_inbox_message(
                                inbox_id.l1_level,
                                inbox_id.l1_message_id,
                                &payload,
                            );
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
    use std::path::Path;

    use jstz_kernel::inbox::encode_signed_operation;
    use jstz_proto::operation::internal::InboxId;
    use octez_riscv::stepper::StepperStatus;
    use tempfile::TempDir;
    use tezos_crypto_rs::hash::SmartRollupHash;
    use tezos_smart_rollup::types::SmartRollupAddress;

    use crate::sequencer::tests::dummy_signed_op;

    #[test]
    #[cfg_attr(
        not(feature = "riscv_test"),
        ignore = "PVM consumes too much memory and therefore this cannot be part of CI"
    )]
    fn create_pvm() {
        let rollup_address =
            SmartRollupHash::from_base58_check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let tmp_dir = TempDir::new().unwrap();
        let riscv_kernel_path =
            Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/riscv_kernel");

        let mut pvm = super::JstzRiscvPvm::new(
            &riscv_kernel_path,
            &rollup_address,
            0,
            Some(tmp_dir.path().to_path_buf().into_boxed_path()),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        let message = encode_signed_operation(
            &dummy_signed_op(),
            &SmartRollupAddress::new(rollup_address),
        )
        .unwrap();
        let output = pvm.execute_operation(
            InboxId {
                l1_level: 123,
                l1_message_id: 456,
            },
            message,
            std::ops::Bound::Unbounded,
        );
        println!("output: {output:?}");
        assert!(matches!(
            output,
            StepperStatus::Exited {
                steps: _,
                success: true,
                status: _
            }
        ));
    }
}
