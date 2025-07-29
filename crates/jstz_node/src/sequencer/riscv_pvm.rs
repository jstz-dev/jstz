use log::warn;
use octez_riscv::{
    machine_state::block_cache::{block, DefaultCacheConfig},
    program::Program,
    pvm::{hooks::PvmHooks, Pvm, PvmStatus},
    state_backend::owned_backend::Owned,
    stepper::{pvm::reveals::RevealRequestResponseMap, StepperStatus},
};
use tezos_crypto_rs::hash::SmartRollupHash;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::InboxMessage;
use tezos_smart_rollup::{inbox::ExternalMessageFrame, types::SmartRollupAddress};

use std::{
    io::{stdout, Write},
    ops::Bound,
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, mpsc::TryRecvError, Arc, RwLock},
};

use crate::sequencer::{
    inbox::parsing::{Message as JstzMessage, ParsedInboxMessage, RollupType},
    queue::OperationQueue,
    worker::write_heartbeat,
};

type MemoryConfig = octez_riscv::machine_state::memory::M32G;
type BlockImplInner = block::Jitted<block::OutlineCompiler<MemoryConfig>, MemoryConfig>;

pub struct JstzPvm {
    pvm: Pvm<MemoryConfig, DefaultCacheConfig, BlockImplInner, Owned>,
    hooks: DebugLogHook,
    rollup_address: SmartRollupHash,
    _origination_level: u32,
    reveal_request_response_map: RevealRequestResponseMap,
    queue: Arc<RwLock<OperationQueue>>,
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
        queue: Arc<RwLock<OperationQueue>>,
        kernel_path: PathBuf,
        rollup_address: SmartRollupHash,
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
            queue,
            hooks,
            rollup_address,
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

            PvmStatus::WaitingForInput => {
                let v = {
                    match self.queue.write() {
                        Ok(mut q) => q.pop(),
                        Err(e) => {
                            warn!("worker failed to read from queue: {e:?}");
                            None
                        }
                    }
                };
                match v {
                    Some(ParsedInboxMessage::JstzMessage(JstzMessage::External(
                        signed_op,
                    ))) => {
                        let bytes =
                            bincode::encode_to_vec(&signed_op, bincode::config::legacy())
                                .unwrap();
                        let mut external = Vec::with_capacity(bytes.len() + 21);

                        let frame = ExternalMessageFrame::Targetted {
                            contents: bytes,
                            address: SmartRollupAddress::new(self.rollup_address.clone()),
                        };

                        frame.bin_write(&mut external).unwrap();

                        let message = InboxMessage::External::<RollupType>(&external);
                        let mut result = Vec::new();
                        message
                            .serialize(&mut result)
                            .expect("serialization of message failed");
                        let success = self.pvm.provide_inbox_message(0, 0, &result);

                        if success {
                            StepperStatus::Running { steps: 1 }
                        } else {
                            StepperStatus::Errored {
                                steps: 0,
                                cause: "PVM was waiting for input".to_owned(),
                                message: "Providing input did not succeed".to_owned(),
                            }
                        }
                    }
                    None => StepperStatus::Exited {
                        steps: 0,
                        success: true,
                        status: "No new message".to_owned(),
                    },
                    v => StepperStatus::Exited {
                        steps: 0,
                        success: false,
                        status: format!("Unknown message {v:?}"),
                    },
                }
            }

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

    pub fn step_max(
        &mut self,
        rx: std::sync::mpsc::Receiver<()>,
        mut step_bounds: Bound<usize>,
    ) -> StepperStatus {
        let mut total_steps = 0usize;

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
                    status: _,
                } => {
                    total_steps = total_steps.saturating_add(steps);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    match rx.try_recv() {
                        Ok(_) | Err(TryRecvError::Disconnected) => {
                            break StepperStatus::Exited {
                                steps: total_steps,
                                success,
                                status: "terminated by kill signal".to_owned(),
                            };
                        }
                        Err(TryRecvError::Empty) => {}
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
    use std::{
        path::PathBuf,
        str::FromStr,
        sync::{mpsc::channel, Arc, RwLock},
    };

    use tezos_crypto_rs::hash::SmartRollupHash;

    use crate::sequencer::{queue::OperationQueue, tests::dummy_op};

    #[test]
    fn create_pvm() {
        let mut q = OperationQueue::new(5);
        q.insert(dummy_op()).unwrap();
        q.insert(dummy_op()).unwrap();
        q.insert(dummy_op()).unwrap();
        let q = Arc::new(RwLock::new(q));

        let mut pvm = super::JstzPvm::new(
            q.clone(),
            PathBuf::from_str("/Users/huanchengchang/code/jstz/target/riscv64gc-unknown-linux-musl/release/kernel-executable").unwrap(),
            SmartRollupHash::from_base58_check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap(),
                0,
            Some(PathBuf::from_str("/tmp/t").unwrap().into_boxed_path()),
            Default::default(),
            Default::default()
        )
        .unwrap();
        let (_, rx) = channel();
        pvm.step_max(rx, std::ops::Bound::Unbounded);
    }
}
