use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_proto::operation::internal::{FaDeposit, InboxId};
use jstz_proto::{context::account::Address, Result};
use num_traits::ToPrimitive;
use tezos_smart_rollup::michelson::{ticket::FA2_1Ticket, MichelsonContract};
use tezos_smart_rollup::types::{Contract, PublicKeyHash as TezosPublicKeyHash};

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
    inbox_id: InboxId,
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

    use jstz_crypto::smart_function_hash::SmartFunctionHash;
    use jstz_proto::{
        context::account::{Address, Addressable},
        operation::internal::{FaDeposit, InboxId},
    };
    use tezos_smart_rollup::{
        michelson::{
            ticket::{FA2_1Ticket, Ticket},
            MichelsonBytes, MichelsonContract, MichelsonNat, MichelsonOption,
            MichelsonPair,
        },
        types::Contract,
    };

    use crate::parsing::{try_parse_contract, try_parse_fa_deposit};

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
        let inbox_id = InboxId {
            l1_level: 1,
            l1_message_id: 41717,
        };
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
        let inbox_id = InboxId {
            l1_level: 1,
            l1_message_id: 41717,
        };

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
