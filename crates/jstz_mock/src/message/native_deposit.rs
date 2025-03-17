use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    michelson::{MichelsonContract, MichelsonOr, MichelsonPair},
    types::{Contract, PublicKeyHash, SmartRollupAddress},
};

use crate::{
    host::{RollupType, MOCK_RECEIVER, MOCK_SENDER, MOCK_SOURCE, NATIVE_TICKETER},
    parse_ticket,
};

use super::MockInternalMessage;

pub struct MockNativeDeposit {
    pub ticketer: ContractKt1Hash,
    pub sender: ContractKt1Hash,
    pub receiver: Contract,
    pub source: PublicKeyHash,
    pub ticket_amount: u32,
    pub ticket_content: (u32, Option<Vec<u8>>),
    pub smart_rollup: Option<SmartRollupAddress>,
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

impl From<&MockNativeDeposit> for RollupType {
    fn from(val: &MockNativeDeposit) -> Self {
        let ticket = parse_ticket(
            val.ticketer.clone(),
            val.ticket_amount,
            val.ticket_content.clone(),
        );

        MichelsonOr::Left(MichelsonPair(
            MichelsonContract(val.receiver.clone()),
            ticket,
        ))
    }
}

impl MockInternalMessage for &MockNativeDeposit {
    fn source(&self) -> PublicKeyHash {
        self.source.clone()
    }

    fn sender(&self) -> ContractKt1Hash {
        self.sender.clone()
    }

    fn smart_rollup(&self) -> Option<SmartRollupAddress> {
        self.smart_rollup.clone()
    }
}
