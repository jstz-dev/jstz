use deno_core::v8;
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

#[derive(Default)]
pub struct ExecutionTracker {
    next_id: AtomicU8,
    pub(super) executions: Mutex<BTreeMap<u8, v8::IsolateHandle>>,
}

impl ExecutionTracker {
    pub fn add(self: &Arc<Self>, isolate_handle: v8::IsolateHandle) -> ExecutionToken {
        let mut executions = self.executions.lock();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        executions.insert(id, isolate_handle);
        ExecutionToken {
            id,
            tracker: self.clone(),
        }
    }
}

pub struct ExecutionToken {
    id: u8,
    tracker: Arc<ExecutionTracker>,
}

impl Drop for ExecutionToken {
    fn drop(&mut self) {
        let mut executions = self.tracker.executions.lock();
        executions.remove(&self.id);
    }
}
