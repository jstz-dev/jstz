use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

use jstz_core::kv::Transaction;
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_kernel::inbox::*;
use jstz_proto::operation::internal::InboxId;
use jstz_proto::operation::{Content, Operation};
use jstz_runtime::wpt::{init_runtime, TestHarnessReport};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_crypto_rs::hash::SmartRollupHash;
use tezos_smart_rollup_mock::MockHost;

const DEFAULT_TICKETER_ADDRESS: &str = "KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton";

fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<ParsedInboxMessage> {
    let input = match rt.read_input() {
        Ok(input) => match input {
            Some(input) => input,
            None => {
                return None;
            }
        },
        Err(_e) => {
            return None;
        }
    };
    let jstz_rollup_address =
        SmartRollupHash::from_base58_check("sr1BxufbqiHt3dn6ahV6eZk9xBD6XV1fYowr")
            .unwrap();
    let inbox_id = InboxId {
        l1_level: input.level,
        l1_message_id: input.id,
    };
    parse_inbox_message(rt, inbox_id, input.as_ref(), ticketer, &jstz_rollup_address)
}

pub fn run(rt: &mut impl Runtime) {
    let mut tx = Transaction::default();
    tx.begin();
    let ticketer = SmartFunctionHash::from_base58(DEFAULT_TICKETER_ADDRESS).unwrap(); // As we have no deposit operation, ticketer isn't actually used

    let mut source = String::new();

    while let Some(message) = read_message(rt, &ticketer) {
        if let ParsedInboxMessage::JstzMessage(Message::External(signed_operation)) =
            message
        {
            let operation: Operation = signed_operation.into();
            if let Content::DeployFunction(deploy_function) = operation.content {
                if deploy_function.function_code.to_string() == "STOP" {
                    break;
                }
                source += &deploy_function.function_code.to_string();
            }
        }
    }

    let mut host = MockHost::default();
    host.set_debug_handler(std::io::empty());
    let mut js_rt = init_runtime(&mut host, &mut tx);

    let result = js_rt.execute_script("native code", source);

    match result {
        Ok(_) => {
            debug_msg!(rt, "script executed successfully");
        }
        Err(e) => {
            debug_msg!(
                rt,
                "{}",
                format!("script execution failed with panic: {:?}", e)
            );
        }
    };

    let data = js_rt
        .op_state()
        .borrow()
        .borrow::<TestHarnessReport>()
        .clone();

    debug_msg!(
        rt,
        "Test harness report: <REPORT_START>{}<REPORT_END> \n",
        serde_json::to_string(&data).unwrap()
    );
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    run(rt);
}
