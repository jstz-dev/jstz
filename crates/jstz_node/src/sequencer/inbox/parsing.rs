#![allow(dead_code)]
/// This file is imported from the jstz_kernel crate. `jstz_kernel/src/inbox.rs`
/// is the original source. There is some build issue importing the `inbox` module
/// directly, so we copy the relevant parts here.
/// https://linear.app/tezos/issue/JSTZ-627/build-script-issue
use jstz_core::{host::WriteDebug, BinEncodable};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_proto::operation::{
    internal::{Deposit, FaDeposit},
    InternalOperation, SignedOperation,
};
use jstz_proto::{context::account::Address, Result};
use num_traits::ToPrimitive;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tezos_smart_rollup::michelson::ticket::FA2_1Ticket;
use tezos_smart_rollup::michelson::{
    MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonOr,
};
pub use tezos_smart_rollup::{
    inbox::{ExternalMessageFrame, InboxMessage, InternalInboxMessage, Transfer},
    michelson::MichelsonPair,
    prelude::Runtime,
    types::{Contract, PublicKeyHash as TezosPublicKeyHash},
};

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

/// Parse a hex-encoded L1 inbox input message into a jstz operation.
///
/// Every L1 inbox message contains at least 3 internal messages:
/// 1. StartOfLevel - Marks the beginning of a new L1 level
/// 2. InfoPerLevel - Contains information about the previous L1 block
/// 3. EndOfLevel - Marks the end of the current L1 level
///
/// The function returns None in the following cases:
/// - If the message is one of the internal messages listed above
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
) -> Option<Message> {
    let inbox_msg = hex::decode(inbox_msg).ok()?;
    parse_inbox_message(logger, inbox_id, &inbox_msg, ticketer, jstz_rollup_address)
}

fn parse_inbox_message(
    logger: &impl WriteDebug,
    inbox_id: u32,
    inbox_msg: &[u8],
    ticketer: &ContractKt1Hash,
    jstz_rollup_address: &SmartRollupHash,
) -> Option<Message> {
    let (_, message) = InboxMessage::<RollupType>::parse(inbox_msg).ok()?;

    match message {
        InboxMessage::Internal(InternalInboxMessage::StartOfLevel) => {
            // Start of level message pushed by the Layer 1 at the
            // beginning of eavh level.
            logger.write_debug("Internal message: start of level\n");
            None
        }
        InboxMessage::Internal(InternalInboxMessage::InfoPerLevel(info)) => {
            // The "Info per level" messages follows the "Start of level"
            // message and contains information on the previous Layer 1 block.
            logger.write_debug(&format!(
                "Internal message: level info \
                        (block predecessor: {}, predecessor_timestamp: {}\n",
                info.predecessor, info.predecessor_timestamp
            ));
            None
        }
        InboxMessage::Internal(InternalInboxMessage::EndOfLevel) => {
            // The "End of level" message is pushed by the Layer 1
            // at the end of each level.
            logger.write_debug("Internal message: end of level\n");
            None
        }
        InboxMessage::Internal(InternalInboxMessage::Transfer(transfer)) => {
            if jstz_rollup_address != transfer.destination.hash() {
                logger.write_debug(
                    "Internal message ignored because of different smart rollup address",
                );
                return None;
            };
            read_transfer(logger, transfer, ticketer, inbox_id)
        }
        InboxMessage::External(bytes) => match ExternalMessageFrame::parse(bytes) {
            Ok(frame) => match frame {
                ExternalMessageFrame::Targetted { address, contents } => {
                    if jstz_rollup_address != address.hash() {
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
                    }
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
                };
                logger.write_debug("Deposit: {content:?}\n");
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

fn read_external_message(
    logger: &impl WriteDebug,
    bytes: &[u8],
) -> Option<ExternalMessage> {
    let msg = ExternalMessage::decode(bytes).ok()?;
    logger.write_debug("External message: {msg:?}\n");
    Some(msg)
}

pub fn try_parse_contract(contract: &Contract) -> Result<Address> {
    match contract {
        Contract::Implicit(TezosPublicKeyHash::Ed25519(tz1)) => {
            Ok(Address::User(PublicKeyHash::Tz1(tz1.clone().into())))
        }
        Contract::Originated(contract_kt1_hash) => Ok(Address::SmartFunction(
            SmartFunctionHash(contract_kt1_hash.clone().into()),
        )),
        _ => Err(jstz_proto::Error::InvalidAddress),
    }
}

pub fn try_parse_fa_deposit(
    inbox_id: u32,
    ticket: FA2_1Ticket,
    receiver: MichelsonContract,
    proxy_contract: Option<MichelsonContract>,
) -> Result<FaDeposit> {
    let receiver = try_parse_contract(&receiver.0)?;

    let proxy_smart_function = (proxy_contract)
        .map(|c| {
            if let addr @ Address::SmartFunction(_) = try_parse_contract(&c.0)? {
                Ok(addr)
            } else {
                Err(jstz_proto::Error::AddressTypeMismatch)
            }
        })
        .transpose()?;

    let amount = ticket
        .amount()
        .to_u64()
        .ok_or(jstz_proto::Error::TicketAmountTooLarge)?;
    let ticket_hash = ticket.hash()?;

    Ok(FaDeposit {
        inbox_id,
        amount,
        receiver,
        proxy_smart_function,
        ticket_hash,
    })
}

#[cfg(test)]
mod test {
    use jstz_core::host::WriteDebug;
    use jstz_crypto::smart_function_hash::SmartFunctionHash;
    use jstz_proto::context::account::{Address, Addressable};
    use jstz_proto::operation::internal::FaDeposit;
    use jstz_proto::operation::{Content, InternalOperation, Operation};
    use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
    use tezos_smart_rollup::michelson::ticket::{FA2_1Ticket, Ticket};
    use tezos_smart_rollup::michelson::{
        MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonPair,
    };
    use tezos_smart_rollup::types::Contract;

    use crate::sequencer::inbox::parsing::{try_parse_contract, try_parse_fa_deposit};

    use super::{parse_inbox_message_hex, Message};

    const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
    const TICKETER: &str = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";

    struct MockLogger;

    impl WriteDebug for MockLogger {
        fn write_debug(&self, _msg: &str) {}
    }

    #[test]
    fn parse_external_inbox_message() {
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let run_function = "0100c3ea4c18195bcfac262dcb29e3d803ae74681739000000004000000000000000b084122920ce655297b86d29e0115ea7b05fe12a22044c8aeaee0fc506915e9a3d955995aec5095840b744deb77470d6d0388042f06f6264e1b1aeec371b100f00000000200000000000000073c58fbff04bb1bc965986ad626d2a233e630ea253d49e1714a0bc9610c1ef450200000000000000010000002c000000000000006a73747a3a2f2f4b543145573235576b5343616b6f686d436a58486363674d61325a5567564759634851372f03000000000000004745540000000000000000007064080000000000";

        let message =
            parse_inbox_message_hex(&MockLogger, 0, run_function, &ticketer, &jstz)
                .expect("Failed to parse inbox message");

        let Message::External(signed) = message else {
            panic!("Expected external message, got internal message");
        };

        signed.verify().expect("Failed to verify signed operation");
        let operation: Operation = signed.into();
        assert!(
            matches!(operation.content, Content::RunFunction(..)),
            "Expected RunFunction operation, got {:?}",
            operation.content
        );
    }

    #[test]
    fn parse_internal_start_of_level_inbox_message() {
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let start_level = "0001";

        assert!(
            parse_inbox_message_hex(&MockLogger, 0, start_level, &ticketer, &jstz)
                .is_none()
        )
    }

    #[test]
    fn parse_internal_transfer_inbox_message() {
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let deposit = "0000050507070a000000160000c4ecf33f52c7b89168cfef8f350818fee1ad08e807070a000000160146d83d8ef8bce4d8c60a96170739c0269384075a00070707070000030600b0d40354267463f8cf2844e4d8b20a76f0471bcb2137fd0002298c03ed7d454a101eb7022bc95f7e5f41ac78c3ea4c18195bcfac262dcb29e3d803ae74681739";

        let message = parse_inbox_message_hex(&MockLogger, 0, deposit, &ticketer, &jstz)
            .expect("Failed to parse inbox message");

        let Message::Internal(transfer) = message else {
            panic!("Expected external message, got internal message");
        };

        assert!(
            matches!(transfer, InternalOperation::Deposit(..)),
            "Expected Deposit"
        );
    }

    fn jstz_pkh_to_michelson(
        pkh: &jstz_crypto::public_key_hash::PublicKeyHash,
    ) -> MichelsonContract {
        MichelsonContract(Contract::from_b58check(&pkh.to_base58()).unwrap())
    }

    fn jstz_sfh_to_michelson(sfh: &SmartFunctionHash) -> MichelsonContract {
        MichelsonContract(Contract::from_b58check(&sfh.to_base58()).unwrap())
    }

    #[test]
    fn try_parse_fa_deposit_should_pass() {
        let amount = 10;
        let ticket: FA2_1Ticket = Ticket::new(
            Contract::from_b58check("KT1NgXQ6Mwu3XKFDcKdYFS6dkkY3iNKdBKEc").unwrap(),
            MichelsonPair(
                MichelsonNat::from(100_u32),
                MichelsonOption(Some(MichelsonBytes(b"12345".to_vec()))),
            ),
            amount,
        )
        .unwrap();
        let receiver = jstz_pkh_to_michelson(&jstz_mock::account1());
        let proxy_contract = Some(jstz_sfh_to_michelson(&jstz_mock::sf_account1()));
        let inbox_id = 41717;
        let ticket_hash = ticket.hash().unwrap();

        let fa_deposit = try_parse_fa_deposit(inbox_id, ticket, receiver, proxy_contract)
            .expect("Failed to parse michelson fa deposit");
        let expected = FaDeposit {
            inbox_id,
            amount,
            receiver: Address::User(jstz_mock::account1()),
            proxy_smart_function: Some(Address::SmartFunction(jstz_mock::sf_account1())),
            ticket_hash,
        };
        assert_eq!(expected, fa_deposit)
    }

    #[test]
    fn try_parse_fa_deposit_should_fail_for_invalid_proxy_address() {
        let amount = 10;
        let ticket: FA2_1Ticket = Ticket::new(
            Contract::from_b58check("KT1NgXQ6Mwu3XKFDcKdYFS6dkkY3iNKdBKEc").unwrap(),
            MichelsonPair(
                MichelsonNat::from(100_u32),
                MichelsonOption(Some(MichelsonBytes(b"12345".to_vec()))),
            ),
            amount,
        )
        .unwrap();
        let receiver = jstz_pkh_to_michelson(&jstz_mock::account2());
        let proxy_contract = Some(jstz_pkh_to_michelson(&jstz_mock::account1()));
        let inbox_id = 41717;

        let fa_deposit = try_parse_fa_deposit(inbox_id, ticket, receiver, proxy_contract);
        assert!(fa_deposit
            .is_err_and(|e| { matches!(e, jstz_proto::Error::AddressTypeMismatch) }));
    }

    #[test]
    fn try_parse_contract_tz1_should_pass() {
        let value = try_parse_contract(
            &Contract::from_b58check("tz1ha7kscNYSgJ76k5gZD8mhBueCv3gqfMsA").unwrap(),
        )
        .expect("Expected to be parsable");
        assert_eq!("tz1ha7kscNYSgJ76k5gZD8mhBueCv3gqfMsA", value.to_base58());
    }

    #[test]
    fn try_parse_contract_kt1_should_pass() {
        let value = try_parse_contract(
            &Contract::from_b58check("KT1EfTusMLoeCAAGd9MZJn5yKzFr6kJU5U91").unwrap(),
        )
        .expect("Expected to be parsable");
        assert_eq!("KT1EfTusMLoeCAAGd9MZJn5yKzFr6kJU5U91", value.to_base58());
    }

    #[test]
    fn try_parse_contract_tz2_should_fail() {
        try_parse_contract(
            &Contract::from_b58check("tz2DAaeGav4dN7E9M68pjKca8d8NC5b3zotS").unwrap(),
        )
        .expect_err("Expected to fail");
    }

    #[test]
    fn try_parse_contract_tz3_should_fail() {
        try_parse_contract(
            &Contract::from_b58check("tz3fTJbAxj1LQCEKDKmYLWKP6e5vNC9vwvyo").unwrap(),
        )
        .expect_err("Expected to fail");
    }

    #[test]
    fn try_parse_contract_tz4_should_fail() {
        try_parse_contract(
            &Contract::from_b58check("tz4DWZXsrP3bdPaZ5B3M3iLVoRMAyxw9oKLH").unwrap(),
        )
        .expect_err("Expected to fail");
    }
}
