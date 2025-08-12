use jstz_core::{host::WriteDebug, BinEncodable};
use jstz_crypto::hash::Hash;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Address;
use jstz_proto::operation::{internal::Deposit, InternalOperation, SignedOperation};
use num_traits::ToPrimitive;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tezos_smart_rollup::inbox::InfoPerLevel;
use tezos_smart_rollup::michelson::ticket::FA2_1Ticket;
use tezos_smart_rollup::michelson::{
    MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonOr,
};
pub use tezos_smart_rollup::{
    inbox::{ExternalMessageFrame, InboxMessage, InternalInboxMessage, Transfer},
    michelson::MichelsonPair,
    prelude::{debug_msg, Runtime},
    types::{self, Contract},
};

use crate::parsing::try_parse_fa_deposit;

pub type ExternalMessage = SignedOperation;
pub type InternalMessage = InternalOperation;

#[derive(Debug, PartialEq, Eq, Clone)]
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

pub fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<Message> {
    let input = rt.read_input().ok()??;
    let _ = rt.mark_for_reboot();
    let jstz_rollup_address = rt.reveal_metadata().address();
    match parse_inbox_message(
        rt,
        input.id,
        input.as_ref(),
        ticketer,
        &jstz_rollup_address,
    )? {
        ParsedInboxMessage::JstzMessage(message) => Some(message),
        _ => None,
    }
}

/// Parse a hex-encoded L1 inbox input message into a jstz operation.
///
/// Every L1 inbox message contains at least 3 internal messages:
/// 1. StartOfLevel - Marks the beginning of a new L1 level
/// 2. InfoPerLevel - Contains information about the previous L1 block
/// 3. EndOfLevel - Marks the end of the current L1 level
///
/// The function returns None in the following cases:
/// - If the message is not targeting the provided `jstz_rollup_address`
/// - For native deposit transfers, if the ticket doesn't come from the provided `ticketer`
///
/// # Arguments
/// * `logger` - Debug logger for tracing message processing
/// * `inbox_id` - The message index in the rollup inbox
/// * `inbox_msg` - The hex-encoded inbox message content
/// * `ticketer` - The L1 ticketer used by the bridge contract for the native deposit
/// * `jstz_rollup_address` - The smart rollup address
pub fn parse_inbox_message_hex(
    logger: &impl WriteDebug,
    inbox_id: u32,
    inbox_msg: &str,
    ticketer: &ContractKt1Hash,
    jstz_rollup_address: &SmartRollupHash,
) -> Option<ParsedInboxMessage> {
    let inbox_msg = hex::decode(inbox_msg).ok()?;
    parse_inbox_message(logger, inbox_id, &inbox_msg, ticketer, jstz_rollup_address)
}

pub fn parse_inbox_message(
    logger: &impl WriteDebug,
    inbox_id: u32,
    inbox_msg: &[u8],
    ticketer: &ContractKt1Hash,
    jstz_rollup_address: &SmartRollupHash,
) -> Option<ParsedInboxMessage> {
    let (_, message) = InboxMessage::<RollupType>::parse(inbox_msg).ok()?;

    match message {
        InboxMessage::Internal(InternalInboxMessage::StartOfLevel) => {
            // Start of level message pushed by the Layer 1 at the
            // beginning of eavh level.
            logger.write_debug("Internal message: start of level\n");
            Some(LevelInfo::Start.into())
        }
        InboxMessage::Internal(InternalInboxMessage::InfoPerLevel(info)) => {
            // The "Info per level" messages follows the "Start of level"
            // message and contains information on the previous Layer 1 block.
            logger.write_debug(&format!(
                "Internal message: level info \
                        (block predecessor: {}, predecessor_timestamp: {}\n",
                info.predecessor, info.predecessor_timestamp
            ));
            Some(LevelInfo::Info(info).into())
        }
        InboxMessage::Internal(InternalInboxMessage::EndOfLevel) => {
            // The "End of level" message is pushed by the Layer 1
            // at the end of each level.
            logger.write_debug("Internal message: end of level\n");
            Some(LevelInfo::End.into())
        }
        InboxMessage::Internal(InternalInboxMessage::Transfer(transfer)) => {
            if jstz_rollup_address != transfer.destination.hash() {
                logger.write_debug(
                    "Internal message ignored because of different smart rollup address",
                );
                return None;
            };
            read_transfer(logger, transfer, ticketer, inbox_id).map(|m| m.into())
        }
        InboxMessage::External(bytes) => match ExternalMessageFrame::parse(bytes) {
            Ok(frame) => match frame {
                ExternalMessageFrame::Targetted { address, contents } => {
                    let message = if jstz_rollup_address != address.hash() {
                        logger.write_debug(
                            "External message ignored because of different smart rollup address",
                        );
                        None
                    } else {
                        match read_external_message(logger, contents) {
                            Some(msg) => Some(Message::External(msg)),
                            None => {
                                logger.write_debug(
                                    "Failed to parse the external message\n",
                                );
                                None
                            }
                        }
                    };
                    message.map(|m| m.into())
                }
            },
            Err(_) => {
                logger.write_debug("Failed to parse the external message frame\n");
                None
            }
        },
    }
}

fn is_valid_native_deposit(
    logger: &impl WriteDebug,
    ticket: &FA2_1Ticket,
    native_ticketer: &ContractKt1Hash,
) -> bool {
    let creator = ticket.creator();
    let contents = ticket.contents();
    match &creator.0 {
        Contract::Originated(kt1) if kt1 == native_ticketer => (),
        _ => {
            logger.write_debug("Deposit ignored because of different ticketer");
            return false;
        }
    };

    let native_ticket_id = MichelsonNat::from(NATIVE_TICKET_ID);
    if contents.0 != native_ticket_id {
        logger.write_debug("Deposit ignored because of different ticket id");
        return false;
    }

    if contents.1 != NATIVE_TICKET_CONTENT {
        logger.write_debug("Deposit ignored because of different ticket content");
        return false;
    }

    true
}

fn read_transfer(
    logger: &impl WriteDebug,
    transfer: Transfer<RollupType>,
    ticketer: &ContractKt1Hash,
    inbox_id: u32,
) -> Option<Message> {
    logger.write_debug("Internal message: transfer\n");
    let source = match PublicKeyHash::from_base58(&transfer.source.to_b58check()) {
        Ok(addr) => addr,
        Err(e) => {
            logger.write_debug(&format!("Failed to parse transfer source: {e:?}\n"));
            return None;
        }
    };
    match transfer.payload {
        MichelsonOr::Left(tez_ticket) => {
            let ticket = tez_ticket.1;

            if is_valid_native_deposit(logger, &ticket, ticketer) {
                let amount = ticket.amount().to_u64()?;
                let address = tez_ticket.0 .0.to_b58check();
                let receiver = Address::from_base58(&address).ok()?;
                let content = Deposit {
                    inbox_id,
                    amount,
                    receiver,
                    source,
                };
                logger.write_debug(format!("Deposit: {content:?}\n").as_str());
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
                try_parse_fa_deposit(inbox_id, ticket, source, receiver, proxy).ok()?;
            Some(Message::Internal(InternalMessage::FaDeposit(fa_deposit)))
        }
    }
}

fn read_external_message(
    logger: &impl WriteDebug,
    bytes: &[u8],
) -> Option<ExternalMessage> {
    let msg = ExternalMessage::decode(bytes).ok()?;
    logger.write_debug(&format!("External message: {msg:?}\n"));
    Some(msg)
}

#[derive(Debug, Clone, derive_more::From)]
pub enum ParsedInboxMessage {
    JstzMessage(Message),
    LevelInfo(LevelInfo),
}

#[derive(Debug, Clone)]
pub enum LevelInfo {
    // Start of level
    Start,
    Info(InfoPerLevel),
    End,
}

#[cfg(test)]
mod test {
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
    use jstz_mock::{
        host::JstzMockHost,
        message::{fa_deposit::MockFaDeposit, native_deposit::MockNativeDeposit},
    };
    use jstz_proto::{
        context::account::{Address, Addressable},
        operation::internal,
    };
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
        if let Message::Internal(InternalMessage::Deposit(internal::Deposit {
            amount,
            receiver,
            source,
            ..
        })) =
            read_message(host.rt(), &ticketer).expect("Expected message but non received")
        {
            assert_eq!(amount, 100);
            assert_eq!(receiver.to_base58(), deposit.receiver.to_b58check());
            assert_eq!(
                Addressable::to_base58(&source),
                deposit.source.to_b58check()
            );
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

        if let Message::Internal(InternalMessage::FaDeposit(internal::FaDeposit {
            amount,
            receiver,
            proxy_smart_function,
            source,
            ..
        })) = read_message(host.rt(), &ticketer).expect("Expected FA message")
        {
            assert_eq!(300, amount);
            assert_eq!(fa_deposit.receiver.to_b58check(), receiver.to_base58());
            assert_eq!(
                fa_deposit.source.to_b58check(),
                Addressable::to_base58(&source),
            );
            assert_eq!(
                Some(
                    SmartFunctionHash::from_base58(jstz_mock::host::MOCK_PROXY).unwrap()
                ),
                proxy_smart_function.map(|p| {
                    match p {
                        Address::User(_) => panic!("Unexpected proxy"),
                        Address::SmartFunction(sfh) => sfh,
                    }
                })
            );
        } else {
            panic!("Expected deposit message")
        }
    }
}
