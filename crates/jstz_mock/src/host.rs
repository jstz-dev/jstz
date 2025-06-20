use std::io::empty;

use jstz_core::{host::HostRuntime, kv::Storage, BinEncodable};

use crate::message::MockInternalMessage;
use derive_more::{Deref, DerefMut};
use jstz_crypto::{public_key::PublicKey, smart_function_hash::SmartFunctionHash};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    inbox::ExternalMessageFrame,
    michelson::{
        ticket::FA2_1Ticket, MichelsonContract, MichelsonOption, MichelsonOr,
        MichelsonPair,
    },
    storage::path::RefPath,
    types::SmartRollupAddress,
};
use tezos_smart_rollup_mock::{MockHost, TransferMetadata};

// L1 Ticketer contract
pub const NATIVE_TICKETER: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
// Large payload injector
pub const INJECTOR: &str = "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav";
// Account that initiated the deposit on L1
pub const MOCK_SOURCE: &str = "tz1WXDeZmSpaCCJqes9GknbeUtdKhJJ8QDA2";
// Account to receive deposits on Jstz
pub const MOCK_RECEIVER: &str = "tz1PCXYfph1FQBy1jBEXVAhzgzoBww4vkjC8";
// Bridge contract that routes deposits to Jstz
pub const MOCK_SENDER: &str = "KT1R7WEtNNim3YgkxPt8wPMczjH3eyhbJMtz";
// Proxy smart function address that handles FA deposits
pub const MOCK_PROXY: &str = "KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton";
// Smart function code that handles the FA deposit callbacks
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
pub const INJECTOR_PATH: RefPath = RefPath::assert_from(b"/injector");
pub type RollupType = MichelsonOr<
    MichelsonPair<MichelsonContract, FA2_1Ticket>,
    MichelsonPair<
        MichelsonContract,
        MichelsonPair<MichelsonOption<MichelsonContract>, FA2_1Ticket>,
    >,
>;

// Wrapper over Mockhost to simplify setup of mock scenarios
#[derive(Deref, DerefMut)]
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

    pub fn add_external_message<T: BinEncodable>(&mut self, message: T) {
        let message = message.encode().unwrap();
        let external_message = ExternalMessageFrame::Targetted {
            address: SmartRollupAddress::new(self.0.reveal_metadata().address()),
            contents: message,
        };
        self.0.add_external(external_message);
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
        let injector: PublicKey = PublicKey::from_base58(INJECTOR).unwrap();
        Storage::insert(&mut mock_host, &INJECTOR_PATH, &injector)
            .expect("Could not insert ticketer");
        mock_host.set_debug_handler(empty());
        Self(mock_host)
    }
}
