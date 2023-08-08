use std::rc::Rc;

use boa_gc::{empty_trace, Finalize, Trace};
use tezos_smart_rollup_host::runtime::{Runtime, ValueType};

#[derive(PartialEq, Eq, Debug)]
pub struct Host<H>(Rc<H>);

impl<H> Host<H> {
    fn get_ref<'b>(&'b self) -> &'b H {
        let Host(rc) = self;
        rc
    }
    fn get_mut<'b>(&'b mut self) -> &'b mut H {
        let rc: &mut Rc<H> = &mut self.0;
        Rc::get_mut(rc).unwrap()
    }
    pub unsafe fn new(host: &mut H) -> Self {
        let ptr: *mut H = host;
        let inner = unsafe { Rc::from_raw(ptr) };
        Self(inner)
    }
}
impl<'a, H> Finalize for Host<H> {
    fn finalize(&self) {}
}
unsafe impl<H> Trace for Host<H> {
    empty_trace!();
}
impl<H> Clone for Host<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Runtime> Runtime for Host<H> {
    fn write_debug(&self, msg: &str) {
        self.get_ref().write_debug(msg);
    }

    fn store_copy(
        &mut self,
        from_path: &impl tezos_smart_rollup::storage::path::Path,
        to_path: &impl tezos_smart_rollup::storage::path::Path,
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_copy(from_path, to_path)
    }
    fn store_count_subkeys<T: tezos_smart_rollup::storage::path::Path>(
        &self,
        prefix: &T,
    ) -> Result<u64, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_count_subkeys(prefix)
    }
    fn last_run_aborted(&self) -> Result<bool, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().last_run_aborted()
    }
    fn store_delete<T: tezos_smart_rollup::storage::path::Path>(
        &mut self,
        path: &T,
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_delete(path)
    }
    fn store_has<T: tezos_smart_rollup::storage::path::Path>(
        &self,
        path: &T,
    ) -> Result<Option<ValueType>, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_has(path)
    }
    fn store_move(
        &mut self,
        from_path: &impl tezos_smart_rollup::storage::path::Path,
        to_path: &impl tezos_smart_rollup::storage::path::Path,
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_move(from_path, to_path)
    }
    fn store_read<T: tezos_smart_rollup::storage::path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_read(path, from_offset, max_bytes)
    }
    fn store_read_all(
        &self,
        path: &impl tezos_smart_rollup::storage::path::Path,
    ) -> Result<Vec<u8>, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_read_all(path)
    }
    fn store_read_slice<T: tezos_smart_rollup::storage::path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        buffer: &mut [u8],
    ) -> Result<usize, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_read_slice(path, from_offset, buffer)
    }
    fn store_value_size(
        &self,
        path: &impl tezos_smart_rollup::storage::path::Path,
    ) -> Result<usize, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().store_value_size(path)
    }
    fn store_write<T: tezos_smart_rollup::storage::path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
        at_offset: usize,
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_write(path, src, at_offset)
    }
    fn store_write_all<T: tezos_smart_rollup::storage::path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_write_all(path, src)
    }
    fn store_delete_value<T: tezos_smart_rollup::storage::path::Path>(
        &mut self,
        path: &T,
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().store_delete_value(path)
    }
    fn write_output(
        &mut self,
        from: &[u8],
    ) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().write_output(from)
    }
    fn read_input(
        &mut self,
    ) -> Result<
        Option<tezos_smart_rollup::types::Message>,
        tezos_smart_rollup::host::RuntimeError,
    > {
        self.get_mut().read_input()
    }
    fn reveal_metadata(&self) -> tezos_smart_rollup::types::RollupMetadata {
        self.get_ref().reveal_metadata()
    }
    fn reveal_preimage(
        &self,
        hash: &[u8; tezos_smart_rollup::core_unsafe::PREIMAGE_HASH_SIZE],
        destination: &mut [u8],
    ) -> Result<usize, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().reveal_preimage(hash, destination)
    }

    fn mark_for_reboot(&mut self) -> Result<(), tezos_smart_rollup::host::RuntimeError> {
        self.get_mut().mark_for_reboot()
    }
    fn upgrade_failed(&self) -> Result<bool, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().upgrade_failed()
    }
    fn restart_forced(&self) -> Result<bool, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().restart_forced()
    }
    fn reboot_left(&self) -> Result<u32, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().reboot_left()
    }
    fn runtime_version(&self) -> Result<String, tezos_smart_rollup::host::RuntimeError> {
        self.get_ref().runtime_version()
    }
}
