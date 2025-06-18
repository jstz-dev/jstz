use bincode::{Decode, Encode};
use derive_more::{Display, Error, From};
use jstz_core::{
    host::HostRuntime,
    kv::{outbox::OutboxMessage, Transaction},
};
use jstz_crypto::smart_function_hash::Kt1Hash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tezos_smart_rollup::{
    michelson::{
        ticket::{FA2_1Ticket, TicketHash},
        MichelsonBytes, MichelsonOption, MichelsonPair,
    },
    types::Contract,
};
use utoipa::ToSchema;

use crate::{
    context::{
        account::{Address, Addressable, Amount},
        ticket_table::TicketTable,
    },
    Error, HttpBody, Result,
};

const WITHDRAW_ENTRYPOINT: &str = "withdraw";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct FaWithdraw {
    pub amount: Amount,
    pub routing_info: RoutingInfo,
    pub ticket_info: TicketInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct RoutingInfo {
    pub receiver: Address,
    pub proxy_l1_contract: Kt1Hash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, Encode, Decode)]
pub struct TicketInfo {
    pub id: u32,
    pub content: Option<Vec<u8>>,
    #[bincode(with_serde)]
    pub ticketer: Kt1Hash,
}

impl TicketInfo {
    pub(super) fn to_ticket(&self, amount: Amount) -> Result<Ticket> {
        FA2_1Ticket::new(
            Contract::Originated(self.ticketer.clone().into()),
            MichelsonPair(
                self.id.into(),
                MichelsonOption(self.content.clone().map(MichelsonBytes)),
            ),
            amount,
        )
        .map_err(|_| Error::InvalidTicketType)?
        .try_into()
    }
}

// Internal wrapper over FA2_1Ticket with the hash field cached.
// Computing the hash requires copying ticket content into a new
// buffer which can be costly for large contents. Exposed to super
// for use in test
pub(super) struct Ticket {
    pub value: FA2_1Ticket,
    pub hash: TicketHash,
}

impl TryFrom<FA2_1Ticket> for Ticket {
    type Error = crate::Error;

    fn try_from(value: FA2_1Ticket) -> Result<Self> {
        let hash = value.hash().map_err(|_| Error::InvalidTicketType)?;
        Ok(Self { value, hash })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct FaWithdrawReceipt {
    pub source: Address,
    #[serde(flatten)]
    pub withdrawal: FaWithdraw,
}

impl FaWithdrawReceipt {
    pub fn to_http_body(&self) -> HttpBody {
        Some(String::as_bytes(&json!(&self).to_string()).to_vec())
    }
}

#[derive(Display, Debug, Error, From)]
pub enum FaWithdrawError {
    InvalidTicketInfo,
    ProxySmartFunctionCannotBeSource,
}

fn create_fa_withdrawal_message(
    routing_info: &RoutingInfo,
    ticket: FA2_1Ticket,
) -> Result<OutboxMessage> {
    let receiver_pkh = routing_info.receiver.to_base58();
    let destination = Contract::Originated(routing_info.proxy_l1_contract.clone().into());
    let message = OutboxMessage::new_withdrawal_message(
        &Contract::try_from(receiver_pkh).unwrap(),
        &destination,
        ticket,
        WITHDRAW_ENTRYPOINT,
    )?;
    Ok(message)
}

// Deducts `amount` from the ticket balance of `ticket_owner` for `ticket.hash`
// and pushes a withdraw outbox message to the outbox queue, returning the outbox
// message id.
fn withdraw_from_ticket_owner(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    ticket_owner: &impl Addressable,
    routing_info: &RoutingInfo,
    amount: Amount,
    ticket: Ticket,
) -> Result<()> {
    TicketTable::sub(rt, tx, ticket_owner, &ticket.hash, amount)?;
    let message = create_fa_withdrawal_message(routing_info, ticket.value)?;
    tx.queue_outbox_message(rt, message)?;
    // TODO: https://linear.app/tezos/issue/JSTZ-113/implement-outbox-message-id
    // Implement outbox message id
    Ok(())
}

impl FaWithdraw {
    /// Execute the [FaWithdrawal] request by deducting ticket balance from `source`` and
    /// pushing a withdraw message to the outbox queue. `proxy_l1_contract` is expected to
    /// implement the %withdraw entrypoint. See /jstz/contracts/examples/fa_ticketer/fa_ticketer.mligo.
    ///
    /// Fails if:
    /// * Source account has insufficient funds
    /// * Outbox queue is full
    /// * Amount is zero
    fn fa_withdraw(
        self,
        rt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &impl Addressable,
    ) -> Result<FaWithdrawReceipt> {
        if self.amount == 0 {
            Err(Error::ZeroAmountNotAllowed)?
        }
        let FaWithdraw {
            amount,
            routing_info,
            ticket_info,
        } = &self;
        let ticket = ticket_info.to_ticket(*amount)?;
        withdraw_from_ticket_owner(rt, tx, source, routing_info, *amount, ticket)?;
        Ok(FaWithdrawReceipt {
            source: source.clone().into(),
            withdrawal: self,
        })
    }

    /// Execute the [FaWithdraw] request atomically. See [Self::fa_withdraw].
    /// for implmentation details.
    pub fn execute(
        self,
        rt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &impl Addressable,
        // TODO: https://linear.app/tezos/issue/JSTZ-114/fa-withdraw-gas-calculation
        // Properly consume gas
        _gas_limit: u64,
    ) -> Result<FaWithdrawReceipt> {
        tx.begin();
        let result = self.fa_withdraw(rt, tx, source);
        if result.is_ok() {
            tx.commit(rt)?;
        } else {
            tx.rollback()?;
        }
        result
    }
}

#[cfg(test)]
mod test {
    use tezos_data_encoding::nom::NomReader;
    use tezos_smart_rollup::{
        michelson::MichelsonContract,
        outbox::{OutboxMessageFull, OutboxMessageTransaction},
        types::Entrypoint,
    };
    use tezos_smart_rollup_mock::MockHost;

    use crate::context::ticket_table::TicketTableError;

    use super::*;

    fn create_fa_withdrawal() -> FaWithdraw {
        let ticket_info = TicketInfo {
            id: 1234,
            content: Some(b"random ticket content".to_vec()),
            ticketer: jstz_mock::kt1_account1().into(),
        };
        let routing_info = RoutingInfo {
            receiver: Address::User(jstz_mock::account2()),
            proxy_l1_contract: jstz_mock::kt1_account1().into(),
        };
        FaWithdraw {
            amount: 10,
            routing_info,
            ticket_info,
        }
    }

    #[test]
    fn execute_fa_withdraw_succeeds() {
        let mut rt = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let fa_withdrawal = create_fa_withdrawal();
        let FaWithdraw {
            amount,
            routing_info,
            ticket_info,
        } = fa_withdrawal.clone();
        tx.begin();
        TicketTable::add(
            &mut rt,
            &mut tx,
            &source,
            &fa_withdrawal.ticket_info.clone().to_ticket(1).unwrap().hash,
            100,
        )
        .expect("Adding ticket balance should succeed");
        tx.commit(&mut rt).unwrap();

        tx.begin();
        let fa_withdrawal_receipt_content = fa_withdrawal
            .clone()
            .execute(&mut rt, &mut tx, &source, 100)
            .expect("Should succeed");
        tx.commit(&mut rt).unwrap();
        assert_eq!(
            FaWithdrawReceipt {
                source,
                withdrawal: fa_withdrawal
            },
            fa_withdrawal_receipt_content,
        );

        let level = rt.run_level(|_| {});
        let outbox = rt.outbox_at(level);

        assert_eq!(1, outbox.len());

        for message in outbox.iter() {
            let (_, message) =
                OutboxMessageFull::<OutboxMessage>::nom_read(message).unwrap();
            let parameters = MichelsonPair(
                MichelsonContract(
                    Contract::try_from(routing_info.clone().receiver.to_base58())
                        .unwrap(),
                ),
                ticket_info.clone().to_ticket(amount).unwrap().value,
            );
            assert_eq!(
                message,
                OutboxMessage::Withdrawal(
                    vec![OutboxMessageTransaction {
                        parameters,
                        destination: Contract::Originated(
                            routing_info.clone().proxy_l1_contract.into()
                        ),
                        entrypoint: Entrypoint::try_from(WITHDRAW_ENTRYPOINT.to_string())
                            .unwrap(),
                    }]
                    .into()
                )
                .into()
            );
        }
    }

    #[test]
    fn execute_fa_withdraw_fails_on_insufficient_funds() {
        let mut rt = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let fa_withdrawal = create_fa_withdrawal();

        tx.begin();
        TicketTable::add(
            &mut rt,
            &mut tx,
            &source,
            &fa_withdrawal.ticket_info.clone().to_ticket(1).unwrap().hash,
            5,
        )
        .expect("Adding ticket balance should succeed");
        tx.commit(&mut rt).unwrap();

        let result = fa_withdrawal.execute(&mut rt, &mut tx, &source, 100);
        assert!(matches!(
            result,
            Err(Error::TicketTableError {
                source: TicketTableError::InsufficientFunds
            })
        ));
    }

    #[test]
    fn execute_fa_withdraw_fails_on_zero_amount() {
        let mut rt = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let ticket_info = TicketInfo {
            id: 1234,
            content: Some(b"random ticket content".to_vec()),
            ticketer: jstz_mock::kt1_account1().into(),
        };
        let routing_info = RoutingInfo {
            receiver: Address::User(jstz_mock::account2()),
            proxy_l1_contract: jstz_mock::kt1_account1().into(),
        };
        let fa_withdrawal = FaWithdraw {
            amount: 0,
            routing_info,
            ticket_info,
        };

        tx.begin();
        TicketTable::add(
            &mut rt,
            &mut tx,
            &source,
            &fa_withdrawal.ticket_info.clone().to_ticket(1).unwrap().hash,
            5,
        )
        .expect("Adding ticket balance should succeed");
        tx.commit(&mut rt).unwrap();

        let result = fa_withdrawal.execute(&mut rt, &mut tx, &source, 100);
        assert!(matches!(result, Err(Error::ZeroAmountNotAllowed)));
    }
}
