use std::collections::VecDeque;

use jstz_kernel::inbox::{ParsedInboxMessage, ParsedInboxMessageWrapper};
use jstz_proto::operation::SignedOperation;

/// A wrapper for the actual parsed operations. The original inbox message is attached for
/// operations coming from the rollup inbox.
#[derive(Clone)]
pub enum WrappedOperation {
    FromInbox {
        message: ParsedInboxMessageWrapper,
        original_inbox_message: String,
    },
    FromNode(SignedOperation),
}

impl WrappedOperation {
    pub fn to_message(self) -> ParsedInboxMessage {
        match self {
            WrappedOperation::FromInbox { message, .. } => message.content,
            WrappedOperation::FromNode(v) => {
                ParsedInboxMessage::JstzMessage(jstz_kernel::inbox::Message::External(v))
            }
        }
    }
}

pub struct OperationQueue {
    capacity: usize,
    queue: VecDeque<WrappedOperation>,
}

impl OperationQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            queue: VecDeque::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, op: WrappedOperation) -> anyhow::Result<()> {
        if self.is_full() {
            anyhow::bail!("queue is full")
        } else {
            self.queue.push_back(op);
            Ok(())
        }
    }

    pub fn insert_ref(&mut self, op: &WrappedOperation) -> anyhow::Result<()> {
        if self.is_full() {
            anyhow::bail!("queue is full")
        } else {
            self.queue.push_back(op.clone());
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Option<WrappedOperation> {
        self.queue.pop_front()
    }

    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }

    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use jstz_proto::operation::internal::InboxId;

    use super::OperationQueue;
    use crate::sequencer::{
        queue::WrappedOperation,
        tests::{dummy_op, dummy_signed_op},
    };

    #[test]
    fn new_queue() {
        let q = OperationQueue::new(5);
        assert_eq!(q.queue.capacity(), 5);
        assert_eq!(q.queue.len(), 0);
        assert_eq!(q.capacity, 5);
    }

    #[test]
    fn insert() {
        let mut q = OperationQueue::new(1);
        assert!(q.insert(dummy_op()).is_ok());
        assert_eq!(
            q.insert(dummy_op()).unwrap_err().to_string(),
            "queue is full"
        );
    }

    #[test]
    fn insert_ref() {
        let mut q = OperationQueue::new(1);
        assert!(q.insert_ref(&dummy_op()).is_ok());
        assert_eq!(
            q.insert_ref(&dummy_op()).unwrap_err().to_string(),
            "queue is full"
        );
    }

    #[test]
    fn is_full() {
        let q = OperationQueue::new(0);
        assert!(q.is_full());

        let mut q = OperationQueue::new(1);
        assert!(!q.is_full());
        q.insert(dummy_op()).unwrap();
    }

    #[test]
    fn pop() {
        let mut q = OperationQueue::new(1);
        assert!(q.pop().is_none());
        q.insert(dummy_op()).unwrap();
        assert!(q.pop().is_some());
    }

    #[test]
    fn wrapped_operation_to_message() {
        let op = WrappedOperation::FromInbox {
            message: jstz_kernel::inbox::ParsedInboxMessageWrapper {
                content: jstz_kernel::inbox::ParsedInboxMessage::LevelInfo(
                    jstz_kernel::inbox::LevelInfo::End,
                ),
                inbox_id: InboxId {
                    l1_level: 0,
                    l1_message_id: 0,
                },
            },
            original_inbox_message: "0002".to_string(),
        };
        assert_eq!(
            op.to_message(),
            jstz_kernel::inbox::ParsedInboxMessage::LevelInfo(
                jstz_kernel::inbox::LevelInfo::End,
            )
        );

        let inner = dummy_signed_op();
        let op = WrappedOperation::FromNode(inner.clone());
        assert_eq!(
            op.to_message(),
            jstz_kernel::inbox::ParsedInboxMessage::JstzMessage(
                jstz_kernel::inbox::Message::External(inner),
            )
        );
    }
}
