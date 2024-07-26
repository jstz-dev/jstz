use std::io::empty;

use jstz_core::{host::HostRuntime, kv::Storage};
use jstz_crypto::hash::Blake2b;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    michelson::{
        ticket::{FA2_1Ticket, Ticket},
        MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonPair,
    },
    storage::path::RefPath,
    types::{Contract, PublicKeyHash, SmartRollupAddress},
};
use tezos_smart_rollup_mock::{MockHost, TransferMetadata};

pub const NATIVE_TICKETER: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
pub const MOCK_RECEIVER: &str = "tz1PCXYfph1FQBy1jBEXVAhzgzoBww4vkjC8";
pub const MOCK_SENDER: &str = "KT1R7WEtNNim3YgkxPt8wPMczjH3eyhbJMtz";
pub const MOCK_SOURCE: &str = "tz1WXDeZmSpaCCJqes9GknbeUtdKhJJ8QDA2";

pub const TICKETER_PATH: RefPath = RefPath::assert_from(b"/ticketer");
pub type RollupType = MichelsonPair<MichelsonContract, FA2_1Ticket>;

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

    pub fn add_deposit_message(&mut self, deposit: &MockNativeDeposit) {
        let payload = deposit.to_jstz();
        let mut metadata =
            TransferMetadata::new(deposit.sender.clone(), deposit.source.clone());
        if let Some(smart_rollup) = deposit.smart_rollup.clone() {
            metadata.override_destination(smart_rollup);
        }
        self.0.add_transfer(payload, &metadata)
    }

    pub fn get_ticketer(&mut self) -> ContractKt1Hash {
        ContractKt1Hash::from_base58_check(NATIVE_TICKETER).unwrap()
    }

    pub fn rt(&mut self) -> &mut MockHost {
        &mut self.0
    }
}

impl Default for JstzMockHost {
    fn default() -> Self {
        let mut mock_host = MockHost::default();
        let ticketer = ContractKt1Hash::from_base58_check(NATIVE_TICKETER).unwrap();
        Storage::insert(&mut mock_host, &TICKETER_PATH, &ticketer)
            .expect("Could not insert ticketer");
        mock_host.set_debug_handler(empty());
        Self(mock_host)
    }
}

pub struct MockNativeDeposit {
    pub ticketer: ContractKt1Hash,
    pub sender: ContractKt1Hash,
    pub receiver: Contract,
    pub source: PublicKeyHash,
    pub ticket_amount: u32,
    pub ticket_content: (u32, Option<Vec<u8>>),
    pub smart_rollup: Option<SmartRollupAddress>,
}

impl MockNativeDeposit {
    pub fn to_jstz(&self) -> RollupType {
        let ticket_content = MichelsonPair(
            MichelsonNat::from(self.ticket_content.0),
            MichelsonOption::<MichelsonBytes>(
                self.ticket_content.1.clone().map(MichelsonBytes),
            ),
        );

        let ticket = Ticket::new(
            Contract::Originated(self.ticketer.clone()),
            ticket_content,
            self.ticket_amount,
        )
        .unwrap();

        MichelsonPair(MichelsonContract(self.receiver.clone()), ticket)
    }
}

impl Default for MockNativeDeposit {
    fn default() -> Self {
        Self {
            ticketer: ContractKt1Hash::from_base58_check(NATIVE_TICKETER).unwrap(),
            sender: ContractKt1Hash::from_base58_check(MOCK_SENDER).unwrap(),
            source: PublicKeyHash::from_b58check(MOCK_SOURCE).unwrap(),
            receiver: Contract::Implicit(
                PublicKeyHash::from_b58check(MOCK_RECEIVER).unwrap(),
            ),
            ticket_amount: 100,
            ticket_content: (0, None),
            smart_rollup: None,
        }
    }
}

pub fn account1() -> jstz_crypto::public_key_hash::PublicKeyHash {
    jstz_crypto::public_key_hash::PublicKeyHash::from_base58(
        "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
    )
    .unwrap()
}

pub fn account2() -> jstz_crypto::public_key_hash::PublicKeyHash {
    jstz_crypto::public_key_hash::PublicKeyHash::from_base58(
        "tz1QcqnzZ8pa6VuE4MSeMjsJkiW94wNrPbgX",
    )
    .unwrap()
}

pub fn ticket_hash1() -> Blake2b {
    let data = vec![b'0', b'0', b'0'];
    Blake2b::from(&data)
}
