use crate::error::Result;
use derive_more::{Display, Error, From};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::{
    core_unsafe::MAX_OUTPUT_SIZE,
    michelson::{ticket::FA2_1Ticket, MichelsonContract, MichelsonPair},
    outbox::{
        AtomicBatch, OutboxMessageFull, OutboxMessageTransactionBatch, OutboxQueue,
    },
    prelude::debug_msg,
};

use tezos_data_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};
use tezos_smart_rollup_host::{path::RefPath, runtime::Runtime};

use super::Storage;

const PERSISTENT_OUTBOX_QUEUE_ROOT: RefPath<'static> =
    RefPath::assert_from(b"/outbox/persistent");

const JSTZ_OUTBOX_QUEUE_META: RefPath<'static> = RefPath::assert_from(b"/outbox/meta");

type NativeWithdrawalParameters = MichelsonPair<MichelsonContract, FA2_1Ticket>;

type Withdrawal = OutboxMessageTransactionBatch<NativeWithdrawalParameters>;

#[derive(Debug, HasEncoding, PartialEq)]
pub enum OutboxMessage {
    Withdrawal(Withdrawal),
}

impl AtomicBatch for OutboxMessage {}

impl BinWriter for OutboxMessage {
    fn bin_write(&self, output: &mut Vec<u8>) -> tezos_data_encoding::enc::BinResult {
        match self {
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
pub(crate) struct SnapshotOutboxQueue(Vec<OutboxMessage>);

impl SnapshotOutboxQueue {
    pub fn extend(&mut self, queue: SnapshotOutboxQueue) {
        self.0.extend(queue.0)
    }

    pub fn queue_message(&mut self, message: OutboxMessage) {
        self.0.push(message)
    }
}

/// Wrapper over the kernel sdk's [OutboxQueue] which tracks the queue's
/// current and maximum length. It is implemented as a singleton, that is,
/// only 1 instance of [PersistentOutboxQueueInner] can exists in durable
/// storage dring the lifetime of jstz. This struct should never be used
/// directly outside of [PersistentOutboxQueue].
#[derive(Debug)]
struct PersistentOutboxQueueInner {
    meta: OutboxQueueMeta,
    rollup_outbox_queue: OutboxQueue<'static, RefPath<'static>>,
}

impl PersistentOutboxQueueInner {
    /// Initializes a new [PersistentOutboxQueueInner] instance in durable
    /// storage. Fails if an instance of the outbox queue already exists.
    fn try_new(rt: &mut impl Runtime, max: u32) -> Result<Self> {
        if OutboxQueueMeta::load(rt)?.is_some() {
            Err(OutboxError::OutboxQueueMetaAlreadyExists)?
        }
        let meta = OutboxQueueMeta { len: 0, max };
        meta.save(rt)?;
        let rollup_outbox_queue = OutboxQueue::new(&PERSISTENT_OUTBOX_QUEUE_ROOT, max)?;
        Ok(PersistentOutboxQueueInner {
            meta,
            rollup_outbox_queue,
        })
    }

    fn load(rt: &mut impl Runtime) -> Result<Self> {
        let meta =
            OutboxQueueMeta::load(rt)?.ok_or(OutboxError::OutboxQueueMetaNotFound)?;
        let rollup_outbox_queue =
            OutboxQueue::new(&PERSISTENT_OUTBOX_QUEUE_ROOT, meta.max)?;
        Ok(PersistentOutboxQueueInner {
            meta,
            rollup_outbox_queue,
        })
    }
}

/// A lazily initialized persistent outbox queue which is simply a wrapper over
/// [PersistentOutboxQueueInner]
#[derive(Debug, Default)]
pub(crate) struct PersistentOutboxQueue {
    inner: Option<PersistentOutboxQueueInner>,
}

impl PersistentOutboxQueue {
    fn init_inner(&mut self, rt: &mut impl Runtime) -> Result<()> {
        if self.inner.is_none() {
            let max = u16::MAX as u32;
            let inner = PersistentOutboxQueueInner::load(rt)
                .or_else(|_| PersistentOutboxQueueInner::try_new(rt, max))?;
            self.inner = Some(inner);
        }
        Ok(())
    }

    /// Get or initialize the [PersistentOutboxQueueInner] instance
    fn get_or_init_inner_mut(
        &mut self,
        rt: &mut impl Runtime,
    ) -> Result<&mut PersistentOutboxQueueInner> {
        self.init_inner(rt)?;
        let value = self
            .inner
            .as_mut()
            .expect("Expecteded inner to be initialized");
        Ok(value)
    }

    /// Flushes the outbox queue and updates the queue length.
    /// WARN: Write heavy function
    fn flush(&mut self, rt: &mut impl Runtime) -> Result<u32> {
        let inner = self.get_or_init_inner_mut(rt)?;
        let flushed_count = inner.rollup_outbox_queue.flush_queue(rt) as u32;
        inner.meta.len -= flushed_count;
        Ok(flushed_count)
    }

    /// Pushes an outbox message to the [OutboxQueue] and saves the queue length.
    /// If enqueing many messages in sequence, use [Self::batch_queue_message] instead.
    fn queue_message(
        &mut self,
        rt: &mut impl Runtime,
        message: OutboxMessageFull<OutboxMessage>,
    ) -> Result<()> {
        let inner = self.get_or_init_inner_mut(rt)?;
        inner.rollup_outbox_queue.queue_message(rt, message)?;
        inner.meta.len += 1;
        inner.meta.save(rt)?;
        Ok(())
    }

    /// Pushes outbox messages to the [OutboxQueue] and saves the final queue length
    fn batch_queue_message(
        &mut self,
        rt: &mut impl Runtime,
        outbox_messages: std::vec::IntoIter<OutboxMessage>,
    ) -> Result<()> {
        let inner = self.get_or_init_inner_mut(rt)?;
        for message in outbox_messages {
            inner
                .rollup_outbox_queue
                .queue_message(rt, message)
                .expect("Unexpected error while queueing message"); // Fatal error
            inner.meta.len += 1;
        }
        inner.meta.save(rt)?;
        Ok(())
    }

    pub fn len(&mut self, rt: &mut impl Runtime) -> Result<u32> {
        let inner = self.get_or_init_inner_mut(rt)?;
        Ok(inner.meta.len)
    }

    pub fn max(&mut self, rt: &mut impl Runtime) -> Result<u32> {
        let inner = self.get_or_init_inner_mut(rt)?;
        Ok(inner.meta.max)
    }

    /// Initializes [PersistentOutboxQueueInner] and sets it to nner. Fails if
    /// an instance of the persistent outbox queue already exists in durable
    /// storage
    #[cfg(test)]
    pub fn try_new(rt: &mut impl Runtime, max: u32) -> Result<Self> {
        let inner = PersistentOutboxQueueInner::try_new(rt, max)?;
        Ok(PersistentOutboxQueue { inner: Some(inner) })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OutboxQueueMeta {
    /// Combined queue length of the rollup and snapshots'
    /// outbox queues
    pub len: u32,
    /// Maximum capacity of the rollup outbox queue
    pub max: u32,
}

impl OutboxQueueMeta {
    // FIX: Unfortunately, the RollupOutboxQueue does not expose any methods to
    // read its metadata so use this workaround for now.
    pub fn load(rt: &impl Runtime) -> Result<Option<OutboxQueueMeta>> {
        Storage::get::<OutboxQueueMeta>(rt, &JSTZ_OUTBOX_QUEUE_META)
    }

    pub fn save(&self, rt: &mut impl Runtime) -> Result<()> {
        Storage::insert(rt, &JSTZ_OUTBOX_QUEUE_META, self)
    }
}

/// Writes the outbox message directly to the Runtime outbox
fn write_outbox_message(
    rt: &mut impl Runtime,
    message: &OutboxMessageFull<OutboxMessage>,
) -> Result<()> {
    let mut buffer = Vec::with_capacity(MAX_OUTPUT_SIZE);
    message
        .bin_write(&mut buffer)
        .map_err(|_| OutboxError::OutboxMessageSerializationError)?;
    rt.write_output(&buffer)?;
    Ok(())
}

/// Flushes the outbox queue in the order of the rollup outbox queue
/// then the outbox queue snapshot. The outbox has a maximum capacity
/// of 100 messages per level. Messages that cannot be flushed in the
/// current level are enqueued into the rollup outbox queue for the
/// next flush.
pub(crate) fn flush(
    rt: &mut impl Runtime,
    persistent_queue: &mut PersistentOutboxQueue,
    snapshot_queue: SnapshotOutboxQueue,
) -> Result<()> {
    let mut flushed_count = 0;

    // 1. Flush the existing outbox queue
    flushed_count += persistent_queue.flush(rt)?;

    // 2. Flush the outbox queue snapshot if there is space in the outbox
    let mut outbox_messages = snapshot_queue.0.into_iter();
    for message in outbox_messages.by_ref() {
        let message = message.into();
        match write_outbox_message(rt, &message) {
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
                // TODO: https://linear.app/tezos/issue/JSTZ-78
                // Optimize kernel sdk outbox queue writes
                persistent_queue
                    .queue_message(rt, message)
                    .expect("Unexpected error while queueing message");
                break;
            }
            Err(e) => {
                // This arm is unexpected and probably indicates a bug
                // or cpu/memory degradation.
                debug_msg!(rt, "Error while writing message to outbox: {:?}", e);
                persistent_queue
                    .queue_message(rt, message)
                    .expect("Unexpected error while queueing message");
                break;
            }
        }
    }

    //  3. Enqueue the remaining messages into the outbox queue
    persistent_queue.batch_queue_message(rt, outbox_messages)?;
    if flushed_count > 0 {
        debug_msg!(
            rt,
            "Flush outbox queue (flushed_count: {})\n",
            flushed_count
        );
    }

    Ok(())
}

#[derive(Display, Debug, Error, From)]
pub enum OutboxError {
    /// Outbox reached its maximum capacity
    OutboxQueueFull,
    /// Error while serializing an outbox message.
    /// This is unexpected and probably indicates a bug
    OutboxMessageSerializationError,
    OutboxQueueMetaNotFound,
    OutboxQueueMetaAlreadyExists,
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
        outbox::{OutboxMessageFull, OutboxMessageTransaction},
        types::{Contract, Entrypoint},
    };

    use tezos_smart_rollup_mock::MockHost;

    use crate::kv::outbox::{flush, write_outbox_message, PersistentOutboxQueue};

    use super::{OutboxMessage, SnapshotOutboxQueue};

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
        OutboxMessage::Withdrawal(vec![outbox_tx].into())
    }

    #[test]
    fn flush_empty_outbox_queue_noop() {
        let mut host = MockHost::default();
        let outbox_queue_snapshot = SnapshotOutboxQueue(vec![]);
        let mut persistent_queue = PersistentOutboxQueue::default();
        flush(&mut host, &mut persistent_queue, outbox_queue_snapshot).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(0, outbox.len());
    }

    #[test]
    fn flush_empty_snapshot_flushes_rollup_queue() {
        let mut host = MockHost::default();

        let accounts = [
            PublicKeyHash::digest(b"account1").unwrap(),
            PublicKeyHash::digest(b"account2").unwrap(),
        ];
        let withdrawals: Vec<OutboxMessage> = accounts
            .clone()
            .into_iter()
            .map(|acc| make_withdrawal(&acc))
            .collect();

        let mut persistent_queue = PersistentOutboxQueue::default();
        persistent_queue
            .batch_queue_message(&mut host, withdrawals.into_iter())
            .unwrap();

        let outbox_queue_snapshot = SnapshotOutboxQueue(vec![]);
        flush(&mut host, &mut &mut persistent_queue, outbox_queue_snapshot).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(0, persistent_queue.len(&mut host).unwrap());
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
        let mut persistent_queue = PersistentOutboxQueue::default();
        let accounts = [
            PublicKeyHash::digest(b"account1").unwrap(),
            PublicKeyHash::digest(b"account2").unwrap(),
            PublicKeyHash::digest(b"account3").unwrap(),
            PublicKeyHash::digest(b"account4").unwrap(),
        ];

        for i in 0..2 {
            persistent_queue
                .queue_message(&mut host, make_withdrawal(&accounts[i]).into())
                .unwrap();
        }

        let outbox_queue_snapshot = SnapshotOutboxQueue(
            accounts[2..]
                .iter()
                .map(|acc| make_withdrawal(acc))
                .collect(),
        );

        flush(&mut host, &mut persistent_queue, outbox_queue_snapshot).unwrap();

        assert_eq!(0, persistent_queue.len(&mut host).unwrap());

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
        let mut persistent_queue = PersistentOutboxQueue::default();
        let mut messages: Vec<OutboxMessage> = vec![];
        for i in 0..120 {
            let account =
                PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            messages.push(make_withdrawal(&account))
        }
        let mut messages = messages.into_iter();

        for message in messages.by_ref().take(60) {
            persistent_queue
                .queue_message(&mut host, message.into())
                .unwrap();
        }

        let outbox_queue_snapshot = SnapshotOutboxQueue(messages.take(60).collect());

        flush(&mut host, &mut persistent_queue, outbox_queue_snapshot).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(20, persistent_queue.len(&mut host).unwrap());
        assert_eq!(100, outbox.len());
    }

    #[test]
    fn write_outbox_message_test() {
        let mut host = MockHost::default();
        let withdrawals = [0; 10]
            .map(|_| make_withdrawal(&PublicKeyHash::digest(b"account1").unwrap()))
            .into_iter();

        for withdrawal in withdrawals {
            write_outbox_message(&mut host, &withdrawal.into()).unwrap();
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
            write_outbox_message(&mut host, &withdrawal.into()).unwrap();
        }

        let error = write_outbox_message(&mut host, &withdrawals.next().unwrap().into())
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
            SnapshotOutboxQueue(vec![make_withdrawal(&acc1)]);
        let outbox_queue_snapshot2 = SnapshotOutboxQueue(vec![make_withdrawal(&acc2)]);

        outbox_queue_snapshot1.extend(outbox_queue_snapshot2);

        assert_eq!(2, outbox_queue_snapshot1.0.len());
        assert_eq!(
            vec![make_withdrawal(&acc1), make_withdrawal(&acc2)],
            outbox_queue_snapshot1.0
        );
    }
}
