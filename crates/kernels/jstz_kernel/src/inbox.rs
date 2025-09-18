use std::error::Error;

use jstz_core::{host::WriteDebug, BinEncodable};
use jstz_crypto::hash::Hash;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Address;
use jstz_proto::operation::{
    internal::{Deposit, InboxId},
    InternalOperation, SignedOperation,
};
use num_traits::ToPrimitive;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::InfoPerLevel;
use tezos_smart_rollup::michelson::ticket::FA2_1Ticket;
use tezos_smart_rollup::michelson::{
    MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonOr,
};
use tezos_smart_rollup::types::SmartRollupAddress;
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

// tag + 20 byte address
const EXTERNAL_FRAME_SIZE: usize = 21;
const NATIVE_TICKET_ID: u32 = 0_u32;
const NATIVE_TICKET_CONTENT: MichelsonOption<MichelsonBytes> = MichelsonOption(None);

// We reach None in 3 cases
// 1. No more inputs
// 2. Input targetting the wrong rollup
// 3. Parsing failures
pub fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<ParsedInboxMessageWrapper> {
    let input = rt.read_input().ok()??;
    let jstz_rollup_address = rt.reveal_metadata().address();
    let inbox_id = InboxId {
        l1_level: input.level,
        l1_message_id: input.id,
    };
    parse_inbox_message(rt, inbox_id, input.as_ref(), ticketer, &jstz_rollup_address)
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
    inbox_id: InboxId,
    inbox_msg: &str,
    ticketer: &ContractKt1Hash,
    jstz_rollup_address: &SmartRollupHash,
) -> Option<ParsedInboxMessageWrapper> {
    let inbox_msg = hex::decode(inbox_msg).ok()?;
    parse_inbox_message(logger, inbox_id, &inbox_msg, ticketer, jstz_rollup_address)
}

pub fn parse_inbox_message(
    logger: &impl WriteDebug,
    inbox_id: InboxId,
    inbox_msg: &[u8],
    ticketer: &ContractKt1Hash,
    jstz_rollup_address: &SmartRollupHash,
) -> Option<ParsedInboxMessageWrapper> {
    let (_, message) = InboxMessage::<RollupType>::parse(inbox_msg).ok()?;

    let content = match message {
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
                            &format!(
                                "External message ignored because of different smart rollup address: {:?} != {:?}",
                                jstz_rollup_address, address.hash()
                            ),
                        );
                        None
                    } else {
                        match read_external_message(logger, contents) {
                            Some(msg) => Some(Message::External(msg)),
                            None => {
                                logger.write_debug(&format!(
                                    "Failed to parse the external message: {contents:?}\n"
                                ));
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
    }?;

    Some(ParsedInboxMessageWrapper { inbox_id, content })
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
    inbox_id: InboxId,
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

#[derive(derive_more::From, Debug, PartialEq, Eq, Clone)]
pub struct ParsedInboxMessageWrapper {
    pub inbox_id: InboxId,
    pub content: ParsedInboxMessage,
}

#[derive(derive_more::From, Debug, PartialEq, Eq, Clone)]
pub enum ParsedInboxMessage {
    JstzMessage(Message),
    LevelInfo(LevelInfo),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LevelInfo {
    // Start of level
    Start,
    Info(InfoPerLevel),
    End,
}

/// Encode signed operations (parsed external messages) back to raw inbox messages.
pub fn encode_signed_operation(
    signed_op: &SignedOperation,
    rollup_addr: &SmartRollupAddress,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = signed_op.encode()?;
    let mut external = Vec::with_capacity(bytes.len() + EXTERNAL_FRAME_SIZE);

    let frame = ExternalMessageFrame::Targetted {
        contents: bytes,
        address: rollup_addr.clone(),
    };

    frame.bin_write(&mut external)?;

    let message = InboxMessage::External::<RollupType>(&external);
    let mut buf = Vec::new();
    message.serialize(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod test {
    use jstz_core::host::WriteDebug;
    use jstz_crypto::{
        hash::Hash, public_key::PublicKey, secret_key::SecretKey,
        smart_function_hash::SmartFunctionHash,
    };
    use jstz_mock::{
        host::JstzMockHost,
        message::{fa_deposit::MockFaDeposit, native_deposit::MockNativeDeposit},
    };
    use jstz_proto::{
        context::account::{Address, Addressable, Nonce},
        operation::{
            internal::{self, InboxId},
            Content, DeployFunction, Operation, SignedOperation,
        },
    };
    use tezos_crypto_rs::hash::{ContractKt1Hash, HashTrait};
    use tezos_smart_rollup::types::SmartRollupAddress;

    use crate::inbox::ParsedInboxMessage;

    use super::{read_message, InternalMessage, Message};

    struct DummyLogger;
    impl WriteDebug for DummyLogger {
        fn write_debug(&self, _msg: &str) {}
    }

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
        let message = read_message(host.rt(), &ticketer)
            .expect("Expected message but non received");
        if let ParsedInboxMessage::JstzMessage(Message::Internal(
            InternalMessage::Deposit(internal::Deposit {
                amount,
                receiver,
                source,
                inbox_id,
            }),
        )) = message.content
        {
            assert_eq!(amount, 100);
            assert_eq!(receiver.to_base58(), deposit.receiver.to_b58check());
            assert_eq!(
                Addressable::to_base58(&source),
                deposit.source.to_b58check()
            );
            assert_eq!(message.inbox_id, inbox_id);
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

        if let ParsedInboxMessage::JstzMessage(Message::Internal(
            InternalMessage::FaDeposit(internal::FaDeposit {
                amount,
                receiver,
                proxy_smart_function,
                source,
                ..
            }),
        )) = read_message(host.rt(), &ticketer)
            .expect("Expected FA message")
            .content
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

    #[test]
    fn encode_signed_operation_round_trip() {
        let ticketer_addr =
            ContractKt1Hash::from_base58_check("KT1RycYvM4EVs6BAXWEsGXaAaRqiMP53KT4w")
                .unwrap();
        let rollup_addr =
            SmartRollupAddress::from_b58check("sr1JVr8SmBYRRFq38HZGM7nJUa9VcfwxGSXc")
                .unwrap();

        let op = Operation {
            public_key: PublicKey::from_base58(
                "edpktpymWssZNE88hDVzXYKTFVidwXnkS9bQKrf1wGEz3koYgFeErn",
            )
            .unwrap(),
            nonce: Nonce(0),
            content: Content::DeployFunction(DeployFunction {
                function_code: "code".to_string(),
                account_credit: 0,
            }),
        };
        let hash = op.hash();
        let signed_op = SignedOperation::new(
            SecretKey::from_base58(
                "edsk4WrVheGeqg1VuQkpybAkyqec8si4hhXM6nzxgYgMsNd7V1gvq6",
            )
            .unwrap()
            .sign(hash)
            .unwrap(),
            op,
        );

        let encoded_msg =
            super::encode_signed_operation(&signed_op, &rollup_addr).unwrap();
        let parsed_message = super::parse_inbox_message(
            &DummyLogger,
            InboxId {
                l1_level: 123,
                l1_message_id: 456,
            },
            &encoded_msg,
            &ticketer_addr,
            rollup_addr.hash(),
        )
        .unwrap();
        assert_eq!(
            parsed_message.content,
            ParsedInboxMessage::JstzMessage(Message::External(signed_op))
        );
        assert_eq!(
            parsed_message.inbox_id,
            InboxId {
                l1_level: 123,
                l1_message_id: 456,
            }
        );
    }
}
