use inbox::Message;
#[cfg(feature = "simulation")]
use jstz_core::event::EventPublish;
use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{public_key::PublicKey, smart_function_hash::SmartFunctionHash};
#[cfg(feature = "simulation")]
use jstz_proto::receipt::SimulationReceipt;
use jstz_proto::Result;
use jstz_proto::{executor, receipt::Receipt};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

pub mod inbox;
pub mod parsing;

#[cfg(feature = "riscv_kernel")]
pub mod riscv_kernel;

#[cfg(not(feature = "riscv_kernel"))]
mod wasm_kernel;

pub const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
pub const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

pub(crate) fn read_ticketer(rt: &impl Runtime) -> SmartFunctionHash {
    Storage::get(rt, &TICKETER)
        .ok()
        .flatten()
        .expect("Ticketer not found")
}

pub(crate) fn read_injector(rt: &impl Runtime) -> PublicKey {
    Storage::get(rt, &INJECTOR)
        .ok()
        .flatten()
        .expect("Revealer not found")
}

pub async fn handle_message(
    hrt: &mut impl Runtime,
    message: Message,
    ticketer: &ContractKt1Hash,
    tx: &mut Transaction,
    injector: &PublicKey,
) -> Result<()> {
    match message {
        Message::Internal(internal_operation) => {
            let receipt =
                executor::execute_internal_operation(hrt, tx, internal_operation).await;
            receipt.write(hrt, tx)?
        }
        Message::External(signed_operation) => {
            debug_msg!(hrt, "External operation: {signed_operation:?}\n");
            #[cfg(feature = "simulation")]
            let simulation_request_id = signed_operation
                .simulation_request()
                .as_ref()
                .map(|r| r.request_id);
            let receipt = executor::execute_operation(
                hrt,
                tx,
                signed_operation,
                ticketer,
                injector,
            )
            .await;
            publish_receipt(
                hrt,
                &receipt,
                #[cfg(feature = "simulation")]
                simulation_request_id,
            )?;
            receipt.write(hrt, tx)?
        }
    }
    Ok(())
}

fn publish_receipt(
    rt: &impl Runtime,
    receipt: &Receipt,
    #[cfg(feature = "simulation")] simulation_request_id: Option<u32>,
) -> Result<()> {
    #[cfg(feature = "simulation")]
    if let Some(simulation_request_id) = simulation_request_id {
        let sim_receipt = SimulationReceipt {
            request_id: simulation_request_id,
            receipt: receipt.clone(),
        };
        return Ok(sim_receipt
            .publish_event(rt)
            .map_err(|source| jstz_core::error::Error::EventError { source })?);
    }
    debug_msg!(rt, "Receipt: {receipt:?}\n");
    Ok(())
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    #[cfg(not(feature = "riscv_kernel"))]
    wasm_kernel::run(rt);

    #[cfg(feature = "riscv_kernel")]
    riscv_kernel::run(rt);
}

#[cfg(all(test, feature = "simulation"))]
mod simulation_tests {
    use super::publish_receipt;
    use http::{HeaderMap, StatusCode};
    use jstz_core::event::decode_line;
    use jstz_mock::host::JstzMockHost;
    use jstz_proto::{
        operation::OperationHash,
        receipt::{Receipt, ReceiptContent, RunFunctionReceipt, SimulationReceipt},
        HttpBody,
    };
    use jstz_utils::test_util::DebugLogSink;

    #[test]
    fn publish_receipt_emits_simulation_receipt_event() {
        let mut host = JstzMockHost::default();
        let sink = DebugLogSink::new();
        host.rt().set_debug_handler(sink.clone());

        let request_id = 42u32;

        let run_fn_receipt = RunFunctionReceipt {
            body: HttpBody::empty(),
            status_code: StatusCode::OK,
            headers: HeaderMap::new(),
        };
        let receipt = Receipt::new(
            OperationHash::default(),
            Ok(ReceiptContent::RunFunction(run_fn_receipt)),
        );

        publish_receipt(host.rt(), &receipt, Some(request_id)).unwrap();

        let output = sink.str_content();
        let decoded = decode_line::<SimulationReceipt>(&output).unwrap();
        let expected = SimulationReceipt {
            request_id,
            receipt: receipt.clone(),
        };
        assert_eq!(decoded, expected);
    }
}
