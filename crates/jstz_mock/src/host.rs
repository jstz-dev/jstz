use std::io::empty;

use jstz_core::{host::HostRuntime, kv::Storage};

use jstz_crypto::smart_function_hash::SmartFunctionHash;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    michelson::{
        ticket::FA2_1Ticket, MichelsonContract, MichelsonOption, MichelsonOr,
        MichelsonPair,
    },
    storage::path::RefPath,
};
use tezos_smart_rollup_mock::{MockHost, TransferMetadata};

use crate::message::MockInternalMessage;

pub const NATIVE_TICKETER: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
pub const MOCK_RECEIVER: &str = "tz1PCXYfph1FQBy1jBEXVAhzgzoBww4vkjC8";
pub const MOCK_SENDER: &str = "KT1R7WEtNNim3YgkxPt8wPMczjH3eyhbJMtz";
pub const MOCK_SOURCE: &str = "tz1WXDeZmSpaCCJqes9GknbeUtdKhJJ8QDA2";

pub const MOCK_PROXY: &str = "KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton";
pub const MOCK_PROXY_FUNCTION: &str = r#"
        export default (request) => {
            const url = new URL(request.url)
            if (url.pathname === "/-/deposit") {
                return new Response();
            }
            return Response.error();
        }
        "#;

pub const MOCK_TICKETER: &str = "KT1H28iie4mW9LmmJeYLjH6zkC8wwSmfHf5P";

pub const TICKETER_PATH: RefPath = RefPath::assert_from(b"/ticketer");
pub type RollupType = MichelsonOr<
    MichelsonPair<MichelsonContract, FA2_1Ticket>,
    MichelsonPair<
        MichelsonContract,
        MichelsonPair<MichelsonOption<MichelsonContract>, FA2_1Ticket>,
    >,
>;

// Wrapper over Mockhost to simplify setup of mock scenarios
pub struct JstzMockHost(MockHost);

impl JstzMockHost {
    pub fn new(skip_meta_messages: bool) -> Self {
        let mut mock_host = Self::default();
        if skip_meta_messages {
            // skip the SOL and Level info messages
            mock_host.0.read_input().unwrap();
            mock_host.0.read_input().unwrap();
        }
        mock_host
    }

    pub fn add_internal_message<'a, T>(&mut self, message: &'a T)
    where
        &'a T: Into<RollupType> + MockInternalMessage,
    {
        let payload: RollupType = message.into();
        let mut metadata = TransferMetadata::new(message.sender(), message.source());
        if let Some(smart_rollup) = message.smart_rollup() {
            metadata.override_destination(smart_rollup);
        }
        self.0.add_transfer(payload, &metadata)
    }

    pub fn get_ticketer(&self) -> SmartFunctionHash {
        ContractKt1Hash::from_base58_check(NATIVE_TICKETER)
            .unwrap()
            .into()
    }

    pub fn rt(&mut self) -> &mut MockHost {
        &mut self.0
    }
}

impl Default for JstzMockHost {
    fn default() -> Self {
        let mut mock_host = MockHost::default();
        let ticketer: SmartFunctionHash =
            ContractKt1Hash::from_base58_check(NATIVE_TICKETER)
                .unwrap()
                .into();
        Storage::insert(&mut mock_host, &TICKETER_PATH, &ticketer)
            .expect("Could not insert ticketer");
        mock_host.set_debug_handler(empty());
        Self(mock_host)
    }
}
