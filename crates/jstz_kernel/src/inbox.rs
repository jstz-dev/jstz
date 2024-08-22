use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::operation::{external::Deposit, ExternalOperation, SignedOperation};
use num_traits::ToPrimitive;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::inbox::ExternalMessageFrame;
use tezos_smart_rollup::michelson::ticket::FA2_1Ticket;
use tezos_smart_rollup::michelson::{
    MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonOr,
};
use tezos_smart_rollup::{
    inbox::{InboxMessage, InternalInboxMessage, Transfer},
    michelson::MichelsonPair,
    prelude::{debug_msg, Runtime},
    types::Contract,
};

use crate::parsing::try_parse_fa_deposit;

pub type ExternalMessage = SignedOperation;
pub type InternalMessage = ExternalOperation;

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    External(ExternalMessage),
    Internal(InternalMessage),
}

pub type MichelsonNativeDeposit = MichelsonPair<MichelsonContract, FA2_1Ticket>;

pub type MichelsonFaDeposit = MichelsonPair<
    MichelsonContract,
    MichelsonPair<MichelsonOption<MichelsonContract>, FA2_1Ticket>,
>;

pub type RollupType = MichelsonOr<MichelsonNativeDeposit, MichelsonFaDeposit>;

const NATIVE_TICKET_ID: u32 = 0_u32;
const NATIVE_TICKET_CONTENT: MichelsonOption<MichelsonBytes> = MichelsonOption(None);

fn is_valid_native_deposit(
    rt: &mut impl Runtime,
    ticket: &FA2_1Ticket,
    native_ticketer: &ContractKt1Hash,
) -> bool {
    let creator = ticket.creator();
    let contents = ticket.contents();
    match &creator.0 {
        Contract::Originated(kt1) if kt1 == native_ticketer => (),
        _ => {
            debug_msg!(rt, "Deposit ignored because of different ticketer");
            return false;
        }
    };

    let native_ticket_id = MichelsonNat::from(NATIVE_TICKET_ID);
    if contents.0 != native_ticket_id {
        debug_msg!(rt, "Deposit ignored because of different ticket id");
        return false;
    }

    if contents.1 != NATIVE_TICKET_CONTENT {
        debug_msg!(rt, "Deposit ignored because of different ticket content");
        return false;
    }

    true
}

fn read_transfer(
    rt: &mut impl Runtime,
    transfer: Transfer<RollupType>,
    ticketer: &ContractKt1Hash,
    inbox_id: u32,
) -> Option<Message> {
    debug_msg!(rt, "Internal message: transfer\n");

    match transfer.payload {
        MichelsonOr::Left(tez_ticket) => {
            let ticket = tez_ticket.1;

            if is_valid_native_deposit(rt, &ticket, ticketer) {
                let amount = ticket.amount().to_u64()?;
                let pkh = tez_ticket.0 .0.to_b58check();
                let receiver = PublicKeyHash::from_base58(&pkh).ok()?;
                let content = Deposit {
                    inbox_id,
                    amount,
                    receiver,
                };
                debug_msg!(rt, "Deposit: {content:?}\n");
                Some(Message::Internal(InternalMessage::Deposit(content)))
            } else {
                None
            }
        }
        MichelsonOr::Right(fa_ticket) => {
            let ticket = fa_ticket.1 .1;
            let receiver = fa_ticket.0;
            let proxy = fa_ticket.1 .0 .0;
            let fa_deposit =
                try_parse_fa_deposit(inbox_id, ticket, receiver, proxy).ok()?;
            Some(Message::Internal(InternalMessage::FaDeposit(fa_deposit)))
        }
    }
}

fn read_external_message(rt: &mut impl Runtime, bytes: &[u8]) -> Option<ExternalMessage> {
    let msg: ExternalMessage = bincode::deserialize(bytes).ok()?;
    debug_msg!(rt, "External message: {msg:?}\n");
    Some(msg)
}

pub fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<Message> {
    let input = rt.read_input().ok()??;
    let _ = rt.mark_for_reboot();

    let (_, message) = InboxMessage::<RollupType>::parse(input.as_ref()).ok()?;

    match message {
        InboxMessage::Internal(InternalInboxMessage::StartOfLevel) => {
            // Start of level message pushed by the Layer 1 at the
            // beginning of eavh level.
            debug_msg!(rt, "Internal message: start of level\n");
            None
        }
        InboxMessage::Internal(InternalInboxMessage::InfoPerLevel(info)) => {
            // The "Info per level" messages follows the "Start of level"
            // message and contains information on the previous Layer 1 block.
            debug_msg!(
                rt,
                "Internal message: level info \
                        (block predecessor: {}, predecessor_timestamp: {}\n",
                info.predecessor,
                info.predecessor_timestamp
            );
            None
        }
        InboxMessage::Internal(InternalInboxMessage::EndOfLevel) => {
            // The "End of level" message is pushed by the Layer 1
            // at the end of each level.
            debug_msg!(rt, "Internal message: end of level\n");
            None
        }
        InboxMessage::Internal(InternalInboxMessage::Transfer(transfer)) => {
            if transfer.destination.hash().as_ref()
                != &rt.reveal_metadata().raw_rollup_address
            {
                debug_msg!(
                    rt,
                    "Internal message ignored because of different smart rollup address"
                );
                return None;
            };
            read_transfer(rt, transfer, ticketer, input.id)
        }
        InboxMessage::External(bytes) => match ExternalMessageFrame::parse(bytes) {
            Ok(frame) => match frame {
                ExternalMessageFrame::Targetted { address, contents } => {
                    let metadata = rt.reveal_metadata();
                    let rollup_address = metadata.address();
                    if &rollup_address != address.hash() {
                        debug_msg!(
                          rt,
                            "Skipping message: External message targets another rollup. Expected: {}. Found: {}\n",
                            rollup_address,
                            address.hash()
                        );
                        None
                    } else {
                        match read_external_message(rt, contents) {
                            Some(msg) => Some(Message::External(msg)),
                            None => {
                                debug_msg!(rt, "Failed to parse the external message\n");
                                None
                            }
                        }
                    }
                }
            },
            Err(_) => {
                debug_msg!(rt, "Failed to parse the external message frame\n");
                None
            }
        },
    }
}

#[cfg(test)]
mod test {
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_mock::message::native_deposit::MockNativeDeposit;
    use jstz_mock::{host::JstzMockHost, message::fa_deposit::MockFaDeposit};
    use jstz_proto::operation::external;
    use tezos_crypto_rs::hash::{ContractKt1Hash, HashTrait};
    use tezos_smart_rollup::types::SmartRollupAddress;

    use super::{read_message, InternalMessage, Message};

    #[test]
    fn read_message_ignored_on_different_smart_rollup_address() {
        let mut host = JstzMockHost::new(true);
        let alternative_smart_rollup_address =
            SmartRollupAddress::from_b58check("sr1Ghq66tYK9y3r8CC1Tf8i8m5nxh8nTvZEf")
                .unwrap();
        let deposit = MockNativeDeposit {
            smart_rollup: Some(alternative_smart_rollup_address),
            ..MockNativeDeposit::default()
        };
        host.add_internal_message(&deposit);
        let ticketer = host.get_ticketer();
        let result = read_message(host.rt(), &ticketer);
        assert_eq!(result, None)
    }

    #[test]
    fn read_message_native_deposit_succeeds() {
        let mut host = JstzMockHost::new(true);
        let deposit = MockNativeDeposit::default();
        let ticketer = host.get_ticketer();
        host.add_internal_message(&deposit);
        if let Message::Internal(InternalMessage::Deposit(external::Deposit {
            amount,
            receiver,
            ..
        })) =
            read_message(host.rt(), &ticketer).expect("Expected message but non received")
        {
            assert_eq!(amount, 100);
            assert_eq!(receiver.to_base58(), deposit.receiver.to_b58check())
        } else {
            panic!("Expected deposit message")
        }
    }

    #[test]
    fn read_message_native_deposit_ignored_different_ticketer() {
        let mut host = JstzMockHost::new(true);
        let ticketer = host.get_ticketer();
        let deposit = MockNativeDeposit {
            ticketer: ContractKt1Hash::from_b58check(
                "KT1KRj5VMNmhxobTJBPq7u2kacqbxu9Cntx6",
            )
            .unwrap(),
            ..MockNativeDeposit::default()
        };
        host.add_internal_message(&deposit);
        assert_eq!(read_message(host.rt(), &ticketer), None);
    }

    #[test]
    fn read_message_native_deposit_ignored_different_ticket_id() {
        let mut host = JstzMockHost::new(true);
        let ticketer = host.get_ticketer();
        let deposit = MockNativeDeposit {
            ticket_content: (1, None),
            ..MockNativeDeposit::default()
        };
        host.add_internal_message(&deposit);
        assert_eq!(read_message(host.rt(), &ticketer), None);
    }

    #[test]
    fn read_message_native_deposit_ignored_different_ticket_value() {
        let mut host = JstzMockHost::new(true);
        let ticketer = host.get_ticketer();
        let deposit = MockNativeDeposit {
            ticket_content: (0, Some(b"1234".to_vec())),
            ..MockNativeDeposit::default()
        };
        host.add_internal_message(&deposit);
        assert_eq!(read_message(host.rt(), &ticketer), None);
    }

    #[test]
    fn read_message_fa_deposit_succeeds() {
        let mut host = JstzMockHost::new(true);
        let fa_deposit = MockFaDeposit::default();
        let ticketer = host.get_ticketer();
        host.add_internal_message(&fa_deposit);

        if let Message::Internal(InternalMessage::FaDeposit(external::FaDeposit {
            amount,
            receiver,
            proxy_smart_function,
            ..
        })) = read_message(host.rt(), &ticketer).expect("Expected FA message")
        {
            assert_eq!(300, amount);
            assert_eq!(fa_deposit.receiver.to_b58check(), receiver.to_base58());
            assert_eq!(
                Some(PublicKeyHash::from_base58(&jstz_mock::host::MOCK_PROXY).unwrap()),
                proxy_smart_function
            );
        } else {
            panic!("Expected deposit message")
        }
    }
}
