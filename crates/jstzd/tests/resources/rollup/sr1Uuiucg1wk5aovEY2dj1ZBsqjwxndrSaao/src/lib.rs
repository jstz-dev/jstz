// SPDX-FileCopyrightText: 2023 Marigold <contact@marigold.dev>
// SPDX-FileCopyrightText: 2023 Nomadic Labs <contact@nomadic-labs.com>
//
// SPDX-License-Identifier: MIT

use tezos_crypto_rs::hash::SmartRollupHash;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::{
    inbox::{InboxMessage, InternalInboxMessage},
    kernel_entry,
    michelson::{Michelson, MichelsonInt},
    outbox::{OutboxMessage, OutboxMessageTransaction, OutboxMessageTransactionBatch},
    prelude::*,
    types::{Contract, Entrypoint},
};

const L1_CONTRACT_ADDRESS: &str = "KT1TFAweS9bMBetdDB3ndFicJWAEMb8MtSrK";
const L1_CONTRACT_ENTRYPOINT: &str = "default";

fn read_inbox_message<Expr: Michelson>(
    host: &mut impl Runtime,
    own_address: &SmartRollupHash,
) {
    loop {
        match host.read_input() {
            Ok(Some(message)) => {
                let parsed_message = InboxMessage::<Expr>::parse(message.as_ref());
                if let Ok((remaining, InboxMessage::External(_))) = parsed_message {
                    debug_assert!(remaining.is_empty());
                    write_outbox_message(host, MichelsonInt::from(1))
                }
            }
            Ok(None) => break,
            Err(_) => continue,
        }
    }
}

fn write_outbox_message<Expr: Michelson>(host: &mut impl Runtime, payload: Expr) {
    let destination = Contract::from_b58check(L1_CONTRACT_ADDRESS).unwrap();
    let entrypoint = Entrypoint::try_from(L1_CONTRACT_ENTRYPOINT.to_string()).unwrap();
    let transaction = OutboxMessageTransaction {
        parameters: payload,
        destination,
        entrypoint,
    };

    let batch = OutboxMessageTransactionBatch::from(vec![transaction]);
    let message = OutboxMessage::AtomicTransactionBatch(batch);
    let mut output = Vec::default();
    message.bin_write(&mut output).unwrap();
    host.write_output(&output).unwrap();
}

pub fn entry(host: &mut impl Runtime) {
    let own_address = host.reveal_metadata().address();
    read_inbox_message::<MichelsonInt>(host, &own_address);
    host.mark_for_reboot().unwrap();
}

kernel_entry!(entry);
