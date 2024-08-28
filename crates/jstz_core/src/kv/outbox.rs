use crate::error::Result;
use derive_more::{Display, Error, From};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::{
    core_unsafe::MAX_OUTPUT_SIZE,
    michelson::{ticket::FA2_1Ticket, MichelsonContract, MichelsonPair},
    outbox::{
        AtomicBatch, OutboxMessageFull, OutboxMessageTransaction,
        OutboxQueue as KernelSdkOutboxQueue, OUTBOX_QUEUE,
    },
    prelude::debug_msg,
};

use tezos_data_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};
use tezos_smart_rollup_host::{path::RefPath, runtime::Runtime};

use super::Storage;

/// Exposing tezos_smart_rollup::outbox::OUTBOX_QUEUE_ROOT
const ROLLUP_OUTBOX_QUEUE_ROOT: RefPath = RefPath::assert_from(b"/__sdk/outbox");

const OUTBOX_QUEUE_ROOT: RefPath = RefPath::assert_from(b"/outbox");

type RollupOutboxQueue = KernelSdkOutboxQueue<'static, RefPath<'static>>;

type NativeWithdrawalParameters = MichelsonPair<MichelsonContract, FA2_1Ticket>;

// TODO: Might need to use OutboxMessageTransactionBatch for L1 encoding
type Withdrawal = OutboxMessageTransaction<NativeWithdrawalParameters>;

#[derive(Debug, HasEncoding, PartialEq)]
pub enum OutboxMessage {
    Withdrawal(Withdrawal),
}

impl AtomicBatch for OutboxMessage {}

impl BinWriter for OutboxMessage {
    fn bin_write(&self, output: &mut Vec<u8>) -> tezos_data_encoding::enc::BinResult {
        match self {
            // TODO: Might need to use OutboxMessageTransactionBatch serialization for
            // L1 encoding
            OutboxMessage::Withdrawal(withdrawal) => withdrawal.bin_write(output),
        }
    }
}

impl<'a> NomReader<'a> for OutboxMessage {
    fn nom_read(input: &'a [u8]) -> tezos_data_encoding::nom::NomResult<'a, Self> {
        nom::branch::alt((nom::combinator::map(Withdrawal::nom_read, |withdrawal| {
            OutboxMessage::Withdrawal(withdrawal)
        }),))(input)
    }
}

impl From<OutboxMessage> for OutboxMessageFull<OutboxMessage> {
    fn from(message: OutboxMessage) -> Self {
        match message {
            OutboxMessage::Withdrawal(_) => {
                OutboxMessageFull::AtomicTransactionBatch(message)
            }
        }
    }
}

/// Represents a pending outbox queue stored as part of the
/// trasaction's snapshot.
#[derive(Debug, Default)]
pub struct OutboxQueueSnapshot(Vec<OutboxMessage>);

impl OutboxQueueSnapshot {
    pub fn extend(&mut self, queue: OutboxQueueSnapshot) {
        self.0.extend(queue.0)
    }

    pub fn queue_message(&mut self, message: OutboxMessage) {
        self.0.push(message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxQueueMeta {
    /// Current queue length between the rollup and snapshots'
    /// outbox queues
    queue_len: u32,
    /// Maximum capacity of the rollup outbox queue
    max: u32,
}

impl OutboxQueueMeta {
    // FIX: Unfortunately, the RollupOutboxQueue does not expose any methods to
    // read its metadata so use this workaround for now.
    pub fn read_meta(rt: &impl Runtime) -> Result<Option<OutboxQueueMeta>> {
        Storage::get::<OutboxQueueMeta>(rt, &OUTBOX_QUEUE_ROOT)
    }

    fn write_meta(&self, rt: &mut impl Runtime) -> Result<()> {
        Storage::insert(rt, &OUTBOX_QUEUE_ROOT, self)
    }
}

/// An outbox queue that composes outbox queue snapshots
/// and `ROLLUP_OUTBOX_QUEUE` while maintaining the invariants
/// of the kernel outbox.
#[derive(Debug)]
pub struct OutboxQueue {
    /// Metadata for the outbox queue
    meta: OutboxQueueMeta,

    /// Rollup outbox queue
    rollup_outbox_queue: RollupOutboxQueue,
}

impl OutboxQueue {
    fn write_outbox_message(
        rt: &mut impl Runtime,
        message: &OutboxMessageFull<impl AtomicBatch>,
    ) -> Result<()> {
        let mut buffer = Vec::with_capacity(MAX_OUTPUT_SIZE);
        message
            .bin_write(&mut buffer)
            .map_err(|_| OutboxError::OutboxMessageSerializationError)?;
        rt.write_output(&buffer)?;
        Ok(())
    }

    pub fn new(max: u32) -> Self {
        Self {
            meta: OutboxQueueMeta { queue_len: 0, max },
            rollup_outbox_queue: RollupOutboxQueue::new(&ROLLUP_OUTBOX_QUEUE_ROOT, max)
                .unwrap(),
        }
    }

    pub fn queue_len(&self) -> u32 {
        self.meta.queue_len
    }

    pub fn incr_queue_len(&mut self) {
        self.meta.queue_len += 1;
    }

    pub fn max(&self) -> u32 {
        self.meta.max
    }

    /// Flushes the outbox queue in the order of the rollup outbox queue
    /// then the outbox queue snapshot. The outbox has a maximum capacity
    /// of 100 messages per level. Messages that cannot be flushed in the
    /// current level are enqueued into the rollup outbox queue for the
    /// next flush.
    pub fn flush(
        &mut self,
        host: &mut impl Runtime,
        snapshot: OutboxQueueSnapshot,
    ) -> usize {
        // 1. Flush the existing outbox queue
        let mut flushed_count = self.rollup_outbox_queue.flush_queue(host);

        // 2. Flush the outbox queue snapshot if there is space in the outbox
        let mut tezos_outbox_messages =
            snapshot.0.into_iter().map(|message| message.into());

        for message in tezos_outbox_messages.by_ref() {
            match Self::write_outbox_message(host, &message) {
                Ok(()) => {
                    flushed_count += 1;
                    continue;
                }
                Err(crate::Error::HostError {
                    source:
                        tezos_smart_rollup::host::RuntimeError::HostErr(
                            tezos_smart_rollup_host::Error::FullOutbox,
                        ),
                }) => {
                    self.rollup_outbox_queue
                        .queue_message(host, message)
                        .unwrap(); // FIXME: handle error
                    break;
                }
                Err(e) => {
                    // This arm is unexpected and probably indicates a bug
                    // or cpu/memory degradation.
                    debug_msg!(host, "Error while writing message to outbox: {:?}", e);
                    self.rollup_outbox_queue
                        .queue_message(host, message)
                        .unwrap(); // FIXME: handle error
                    break;
                }
            }
        }

        //  3. Enqueue the remaining messages into the outbox queue
        for message in tezos_outbox_messages {
            self.rollup_outbox_queue
                .queue_message(host, message)
                .expect("Should always be able to queue message"); // FIXME: handle error
        }

        if flushed_count > 0 {
            debug_msg!(host, "Flushing outbox queue ({})\n", flushed_count);
        }
        // 4. Finally, update the counter outbox messages
        self.meta.queue_len -= flushed_count as u32;
        self.meta
            .write_meta(host)
            .expect("Should always be able to write OutboxQueueMeta"); // FIXME: handle error
        flushed_count
    }
}

impl Default for OutboxQueue {
    fn default() -> Self {
        Self {
            meta: OutboxQueueMeta {
                queue_len: 0,
                max: u16::MAX as u32,
            },
            rollup_outbox_queue: OUTBOX_QUEUE,
        }
    }
}

#[derive(Display, Debug, Error, From)]
pub enum OutboxError {
    /// Outbox reached its maximum capacity
    OutboxFull,
    /// Error while serializing an outbox message.
    /// This is unexpected and probably indicates a bug
    OutboxMessageSerializationError,
}

#[cfg(test)]
mod test {
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use tezos_data_encoding::nom::NomReader;
    use tezos_smart_rollup::{
        michelson::{
            ticket::FA2_1Ticket, MichelsonContract, MichelsonNat, MichelsonOption,
            MichelsonPair,
        },
        outbox::{OutboxMessageFull, OutboxMessageTransaction, OUTBOX_QUEUE},
        types::{Contract, Entrypoint},
    };

    use tezos_smart_rollup_mock::MockHost;

    use crate::kv::outbox::{OutboxQueueMeta, ROLLUP_OUTBOX_QUEUE_ROOT};

    use super::{OutboxMessage, OutboxQueue, OutboxQueueSnapshot, RollupOutboxQueue};

    fn make_withdrawal(account: &PublicKeyHash) -> OutboxMessage {
        let creator =
            Contract::from_b58check("KT1NgXQ6Mwu3XKFDcKdYFS6dkkY3iNKdBKEc").unwrap();
        let parameters = MichelsonPair(
            MichelsonContract(Contract::try_from(account.to_base58()).unwrap()),
            FA2_1Ticket::new(
                creator.clone(),
                MichelsonPair(MichelsonNat::from(0), MichelsonOption(None)),
                10,
            )
            .unwrap(),
        );
        let outbox_tx = OutboxMessageTransaction {
            parameters,
            destination: creator,
            entrypoint: Entrypoint::try_from("burn".to_string()).unwrap(),
        };
        OutboxMessage::Withdrawal(outbox_tx)
    }

    #[test]
    fn flush_empty_outbox_queue_does_nothing() {
        let mut host = MockHost::default();
        let outbox_queue_snapshot = OutboxQueueSnapshot(vec![]);
        let mut outbox_queue = OutboxQueue::default();

        let num_flushed = outbox_queue.flush(&mut host, outbox_queue_snapshot);

        assert_eq!(0, num_flushed);
        assert_eq!(0, outbox_queue.queue_len());

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(0, outbox.len());
    }

    #[test]
    fn flush_empty_snapshot_flushes_rollup_queue() {
        let mut host = MockHost::default();
        let mut outbox_queue = OutboxQueue::default();
        outbox_queue.meta.queue_len = 2;
        let accounts = [
            PublicKeyHash::digest(b"account1").unwrap(),
            PublicKeyHash::digest(b"account2").unwrap(),
        ];
        let withdrawals: Vec<OutboxMessage> = accounts
            .clone()
            .into_iter()
            .map(|acc| make_withdrawal(&acc))
            .collect();

        for withdrawal in withdrawals {
            OUTBOX_QUEUE.queue_message(&mut host, withdrawal).unwrap();
        }

        let outbox_queue_snapshot = OutboxQueueSnapshot(vec![]);
        let num_flushed = outbox_queue.flush(&mut host, outbox_queue_snapshot);

        assert_eq!(2, num_flushed);
        assert_eq!(0, outbox_queue.queue_len());

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(2, outbox.len());

        for (i, message) in outbox.iter().enumerate() {
            let (_, message) =
                OutboxMessageFull::<OutboxMessage>::nom_read(message).unwrap();
            assert_eq!(message, make_withdrawal(&accounts[i]).into());
        }
    }

    #[test]
    fn flush_rollup_queue_first_then_snapshot_queue() {
        let mut host = MockHost::default();
        let mut outbox_queue = OutboxQueue::default();
        outbox_queue.meta.queue_len = 4;

        let accounts = [
            PublicKeyHash::digest(b"account1").unwrap(),
            PublicKeyHash::digest(b"account2").unwrap(),
            PublicKeyHash::digest(b"account3").unwrap(),
            PublicKeyHash::digest(b"account4").unwrap(),
        ];
        for i in 0..2 {
            outbox_queue
                .rollup_outbox_queue
                .queue_message(&mut host, make_withdrawal(&accounts[i]))
                .unwrap();
        }

        let outbox_queue_snapshot = OutboxQueueSnapshot(
            accounts[2..]
                .iter()
                .map(|acc| make_withdrawal(acc))
                .collect(),
        );

        let num_flushed = outbox_queue.flush(&mut host, outbox_queue_snapshot);

        assert_eq!(0, outbox_queue.queue_len());
        assert_eq!(4, num_flushed);

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(4, outbox.len());

        for (i, message) in outbox.iter().enumerate() {
            let (_, message) =
                OutboxMessageFull::<OutboxMessage>::nom_read(message).unwrap();
            assert_eq!(message, make_withdrawal(&accounts[i]).into());
        }
    }

    #[test]
    fn flush_enqueues_remaining_messages_to_rollup_queue() {
        let mut host = MockHost::default();
        let mut messages: Vec<OutboxMessage> = vec![];
        for i in 0..120 {
            let account =
                PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            messages.push(make_withdrawal(&account))
        }
        let mut messages = messages.into_iter();

        let max = u16::MAX as u32;
        let mut outbox_queue = OutboxQueue {
            meta: OutboxQueueMeta {
                queue_len: 120,
                max,
            },
            rollup_outbox_queue: RollupOutboxQueue::new(&ROLLUP_OUTBOX_QUEUE_ROOT, max)
                .unwrap(),
        };

        for message in messages.by_ref().take(60) {
            OUTBOX_QUEUE.queue_message(&mut host, message).unwrap();
        }

        let outbox_queue_snapshot = OutboxQueueSnapshot(messages.take(60).collect());

        outbox_queue.flush(&mut host, outbox_queue_snapshot);

        assert_eq!(20, outbox_queue.queue_len());

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(100, outbox.len());
    }

    #[test]
    fn write_outbox_message() {
        let mut host = MockHost::default();
        let withdrawals = [0; 10]
            .map(|_| make_withdrawal(&PublicKeyHash::digest(b"account1").unwrap()))
            .into_iter();

        for withdrawal in withdrawals {
            OutboxQueue::write_outbox_message(&mut host, &withdrawal.into()).unwrap();
        }

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(10, outbox.len());
    }

    #[test]
    fn write_outbox_message_errors_on_full_outbox() {
        let mut host = MockHost::default();
        let mut withdrawals = [0; 101]
            .map(|_| make_withdrawal(&PublicKeyHash::digest(b"account1").unwrap()))
            .into_iter();

        for withdrawal in withdrawals.by_ref().take(100) {
            OutboxQueue::write_outbox_message(&mut host, &withdrawal.into()).unwrap();
        }

        let error = OutboxQueue::write_outbox_message(
            &mut host,
            &withdrawals.next().unwrap().into(),
        )
        .expect_err("Expected outbox full error");

        assert!(matches!(
            error,
            crate::Error::HostError {
                source: tezos_smart_rollup::host::RuntimeError::HostErr(
                    tezos_smart_rollup_host::Error::FullOutbox,
                ),
            }
        ));
    }

    #[test]
    fn extend_snapshot() {
        let acc1 = PublicKeyHash::digest(b"account1").unwrap();
        let acc2 = PublicKeyHash::digest(b"account2").unwrap();
        let mut outbox_queue_snapshot1 =
            OutboxQueueSnapshot(vec![make_withdrawal(&acc1)]);
        let outbox_queue_snapshot2 = OutboxQueueSnapshot(vec![make_withdrawal(&acc2)]);

        outbox_queue_snapshot1.extend(outbox_queue_snapshot2);

        assert_eq!(2, outbox_queue_snapshot1.0.len());
        assert_eq!(
            vec![make_withdrawal(&acc1), make_withdrawal(&acc2)],
            outbox_queue_snapshot1.0
        );
    }
}
