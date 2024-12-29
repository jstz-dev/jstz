use jstz_crypto::hash::Hash;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    michelson::{
        ticket::TicketHash, MichelsonContract, MichelsonOption, MichelsonOr,
        MichelsonPair,
    },
    types::{Contract, PublicKeyHash, SmartRollupAddress},
};

use crate::{
    host::{RollupType, MOCK_PROXY},
    parse_ticket,
};

use super::{native_deposit::MockNativeDeposit, MockInternalMessage};

pub struct MockFaDeposit {
    pub ticketer: ContractKt1Hash,
    pub sender: ContractKt1Hash,
    pub receiver: Contract,
    pub source: PublicKeyHash,
    pub ticket_amount: u32,
    pub ticket_content: (u32, Option<Vec<u8>>),
    pub smart_rollup: Option<SmartRollupAddress>,
    pub proxy_contract: Option<jstz_crypto::public_key_hash::PublicKeyHash>, // proxy must be tz1 for nows
}

impl Default for MockFaDeposit {
    fn default() -> Self {
        let MockNativeDeposit {
            ticketer,
            sender,
            source,
            receiver,
            smart_rollup,
            ..
        } = MockNativeDeposit::default();
        Self {
            ticketer,
            sender,
            source,
            receiver,
            ticket_amount: 300,
            ticket_content: (12345, Some(b"123967145810851823941234".to_vec())),
            smart_rollup,
            proxy_contract: Some(
                jstz_crypto::public_key_hash::PublicKeyHash::from_base58(MOCK_PROXY)
                    .unwrap(),
            ),
        }
    }
}

impl MockFaDeposit {
    pub fn ticket_hash(&self) -> TicketHash {
        let ticket = parse_ticket(
            self.ticketer.clone(),
            self.ticket_amount,
            self.ticket_content.clone(),
        );

        ticket.hash().unwrap()
    }
}

impl From<&MockFaDeposit> for RollupType {
    fn from(val: &MockFaDeposit) -> Self {
        let ticket = parse_ticket(
            val.ticketer.clone(),
            val.ticket_amount,
            val.ticket_content.clone(),
        );

        let proxy = val
            .proxy_contract
            .clone()
            .map(|p| MichelsonContract(Contract::from_b58check(&p.to_base58()).unwrap()));

        MichelsonOr::Right(MichelsonPair(
            MichelsonContract(val.receiver.clone()),
            MichelsonPair(MichelsonOption(proxy), ticket),
        ))
    }
}

impl MockInternalMessage for &MockFaDeposit {
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
