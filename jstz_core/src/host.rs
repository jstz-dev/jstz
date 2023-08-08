use std::{cell::RefCell, mem, rc::Rc};

use boa_engine::{
    context::HostHooks, object::builtins::JsFunction, realm::Realm, Context,
    JsNativeError, JsResult,
};
use boa_gc::{empty_trace, Finalize, Trace};
use tezos_smart_rollup_host::{
    input, metadata, path,
    runtime::{Runtime, RuntimeError, ValueType},
};

#[derive(Debug)]
pub struct Host<H: Runtime + 'static> {
    rt: Rc<RefCell<&'static mut H>>,
}

impl<H> Finalize for Host<H> where H: Runtime {}

// SAFETY:
// For all intents and purposes (in jstz), a host is a 'static reference
// that doesn't need to be traced
unsafe impl<H> Trace for Host<H>
where
    H: Runtime,
{
    empty_trace!();
}

// IMPLICIT TRAIT:
// impl<H> NativeObject for Host<H> where H: Runtime + 'static {}

impl<H> Clone for Host<H>
where
    H: Runtime,
{
    fn clone(&self) -> Self {
        Self {
            rt: self.rt.clone(),
        }
    }
}

impl<H> Host<H>
where
    H: Runtime,
{
    pub unsafe fn new(rt: &mut H) -> Host<H> {
        Host {
            rt: Rc::from_raw(mem::transmute(rt)),
        }
    }
}

impl<H> Runtime for Host<H>
where
    H: Runtime,
{
    fn write_output(&mut self, from: &[u8]) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().write_output(from)
    }

    fn write_debug(&self, msg: &str) {
        self.rt.borrow().write_debug(msg)
    }

    fn read_input(&mut self) -> Result<Option<input::Message>, RuntimeError> {
        self.rt.borrow_mut().read_input()
    }

    fn store_has<T: path::Path>(
        &self,
        path: &T,
    ) -> Result<Option<ValueType>, RuntimeError> {
        self.rt.borrow().store_has(path)
    }

    fn store_read<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.rt.borrow().store_read(path, from_offset, max_bytes)
    }

    fn store_read_slice<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        buffer: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        self.rt.borrow().store_read_slice(path, from_offset, buffer)
    }

    fn store_read_all(&self, path: &impl path::Path) -> Result<Vec<u8>, RuntimeError> {
        self.rt.borrow().store_read_all(path)
    }

    fn store_write<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
        at_offset: usize,
    ) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_write(path, src, at_offset)
    }

    fn store_write_all<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
    ) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_write_all(path, src)
    }

    fn store_delete<T: path::Path>(&mut self, path: &T) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_delete(path)
    }

    fn store_delete_value<T: path::Path>(
        &mut self,
        path: &T,
    ) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_delete_value(path)
    }

    fn store_count_subkeys<T: path::Path>(
        &self,
        prefix: &T,
    ) -> Result<u64, RuntimeError> {
        self.rt.borrow().store_count_subkeys(prefix)
    }

    fn store_move(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_move(from_path, to_path)
    }

    fn store_copy(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().store_copy(from_path, to_path)
    }

    fn reveal_preimage(
        &self,
        hash: &[u8; 33],
        destination: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        self.rt.borrow().reveal_preimage(hash, destination)
    }

    fn store_value_size(&self, path: &impl path::Path) -> Result<usize, RuntimeError> {
        self.rt.borrow().store_value_size(path)
    }

    fn mark_for_reboot(&mut self) -> Result<(), RuntimeError> {
        self.rt.borrow_mut().mark_for_reboot()
    }

    fn reveal_metadata(&self) -> metadata::RollupMetadata {
        self.rt.borrow().reveal_metadata()
    }

    fn last_run_aborted(&self) -> Result<bool, RuntimeError> {
        self.rt.borrow().last_run_aborted()
    }

    fn upgrade_failed(&self) -> Result<bool, RuntimeError> {
        self.rt.borrow().upgrade_failed()
    }

    fn restart_forced(&self) -> Result<bool, RuntimeError> {
        self.rt.borrow().restart_forced()
    }

    fn reboot_left(&self) -> Result<u32, RuntimeError> {
        self.rt.borrow().reboot_left()
    }

    fn runtime_version(&self) -> Result<String, RuntimeError> {
        self.rt.borrow().runtime_version()
    }
}

struct Hooks;

impl HostHooks for Hooks {
    fn ensure_can_compile_strings(
        &self,
        _realm: Realm,
        _context: &mut Context<'_>,
    ) -> JsResult<()> {
        Err(JsNativeError::typ()
            .with_message("eval calls not available")
            .into())
    }

    fn has_source_text_available(
        &self,
        _function: &JsFunction,
        _context: &mut Context<'_>,
    ) -> bool {
        false
    }
}

pub const HOOKS: &'static dyn HostHooks = &Hooks;

pub trait Api {
    /// Initialize a JSTZ runtime API
    fn init<H: Runtime>(context: &mut Context<'_>, host: Host<H>);
}
