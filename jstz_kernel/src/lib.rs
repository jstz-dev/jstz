use jstz_core::kv::Storage;
use jstz_proto::Result;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{kernel_entry, prelude::Runtime, storage::path::RefPath};

use crate::{
    apply::{
        apply_deploy_contract, apply_deposit, apply_run_contract, apply_transaction,
    },
    inbox::{read_message, ExternalMessage, InternalMessage, Message},
};

mod apply;
pub mod inbox;

const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");

fn read_ticketer(rt: &impl Runtime) -> Option<ContractKt1Hash> {
    Some(Storage::get(rt, &TICKETER).ok()??)
}

fn handle_message(rt: &mut (impl Runtime + 'static), message: Message) -> Result<()> {
    match message {
        Message::Internal(InternalMessage::Deposit(deposit)) => {
            apply_deposit(rt, deposit)
        }
        Message::External(ExternalMessage::RunContract(run)) => {
            apply_run_contract(rt, run)
        }
        Message::External(ExternalMessage::DeployContract(contract)) => {
            apply_deploy_contract(rt, contract)
        }
        // TODO ⚰️ Deprecate will not be part of the CLI
        Message::External(ExternalMessage::Transaction(tx)) => apply_transaction(rt, tx),
    }
}

// kernel entry
pub fn entry(rt: &mut (impl Runtime + 'static)) {
    let ticketer = read_ticketer(rt);

    if let Some(message) = read_message(rt, ticketer.as_ref()) {
        handle_message(rt, message)
            .unwrap_or_else(|err| rt.write_debug(&format!("[🔴] {err:?}\n")));
    }
}

kernel_entry!(entry);
