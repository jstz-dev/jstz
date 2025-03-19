use jstz_crypto::{
    hash::Hash, public_key::PublicKey, public_key_hash::PublicKeyHash,
    secret_key::SecretKey, smart_function_hash::SmartFunctionHash,
};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    michelson::{
        ticket::{FA2_1Ticket, Ticket, TicketHash, UnitTicket},
        MichelsonBytes, MichelsonNat, MichelsonOption, MichelsonPair, MichelsonUnit,
    },
    types::Contract,
};

pub mod host;
pub mod message;

pub fn parse_ticket(
    ticketer: ContractKt1Hash,
    amount: u32,
    content: (u32, Option<Vec<u8>>),
) -> FA2_1Ticket {
    let ticket_content = MichelsonPair(
        MichelsonNat::from(content.0),
        MichelsonOption::<MichelsonBytes>(content.1.map(MichelsonBytes)),
    );

    Ticket::new(Contract::Originated(ticketer), ticket_content, amount).unwrap()
}

pub fn sk1() -> SecretKey {
    SecretKey::from_base58("edsk3M1HxYgWowRxyQQJTp3Zz9QwpSawRmzdvu7fKzxEgwTmggiRb3")
        .unwrap()
}

pub fn pk1() -> PublicKey {
    PublicKey::from_base58("edpkuifh2JiPVYfEM4LuGBcPjhHR1GS88bc4ciNUqg15UcWM5zjFmn")
        .unwrap()
}

pub fn pkh1() -> PublicKeyHash {
    PublicKeyHash::from_base58("tz1SfjqKLQC2pUTduhSoa8HkcAsuTPgh1fVU").unwrap()
}

pub fn sk2() -> SecretKey {
    SecretKey::from_base58("edsk3tf3TGZEpf7TRZZPVxiWtN7dYwa2zRecme3UCWEDVvzBZEY4go")
        .unwrap()
}

pub fn pk2() -> PublicKey {
    PublicKey::from_base58("edpkuaLhZbGU3noeP82aXejhPJFMYtEWouK2ZEc5i1PraC3t2KjZ2W")
        .unwrap()
}

pub fn pkh2() -> PublicKeyHash {
    PublicKeyHash::from_base58("tz1MX7cSM75S8zvcy2UuUWoWtwZQ5chaGyAh").unwrap()
}

// TODO(): Replace account1 and account2 usage with pk1 and pk2
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

pub fn kt1_account1() -> ContractKt1Hash {
    ContractKt1Hash::try_from("KT1QgfSE4C1dX9UqrPAXjUaFQ36F9eB4nNkV").unwrap()
}

pub fn sf_account1() -> SmartFunctionHash {
    SmartFunctionHash::from_base58("KT1QgfSE4C1dX9UqrPAXjUaFQ36F9eB4nNkV").unwrap()
}

pub fn ticket_hash1() -> TicketHash {
    let ticket = UnitTicket::new(
        Contract::from_b58check("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap(),
        MichelsonUnit,
        10,
    )
    .unwrap();
    ticket.hash().unwrap()
}
