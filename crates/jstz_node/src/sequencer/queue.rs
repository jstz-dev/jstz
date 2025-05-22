use std::collections::VecDeque;

use jstz_proto::operation::SignedOperation;

pub struct OperationQueue {
    capacity: usize,
    queue: VecDeque<SignedOperation>,
}

impl OperationQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            queue: VecDeque::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, op: SignedOperation) -> anyhow::Result<()> {
        if self.is_full() {
            anyhow::bail!("queue is full")
        } else {
            self.queue.push_back(op);
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Option<SignedOperation> {
        self.queue.pop_front()
    }

    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::OperationQueue;
    use crate::sequencer::tests::dummy_op;

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
}
