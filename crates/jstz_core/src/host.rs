use derive_more::Display;
use tezos_smart_rollup_host::{
    dal_parameters::RollupDalParameters,
    input,
    metadata::RollupMetadata,
    path::{self, Path},
    runtime::{Runtime, ValueType},
};

pub use tezos_smart_rollup_host::runtime::{
    Runtime as HostRuntime, RuntimeError as HostError, RevealError
};

mod erased_runtime {
    use super::*;

    mod sealed {
        pub mod runtime {
            pub trait Sealed {}
        }
    }

    /// An object-safe equivalent of a Smart Rollup's `Runtime` trait.
    ///
    /// Any implementation of the `Runtime` trait converts seamlessly to an
    /// `&erased_runtime::Runtime` trait object
    ///
    /// This trait is sealed and can only be implemented via a
    /// `tezos_smart_rollup::runtime::Runtime` impl.
    pub trait Runtime: sealed::runtime::Sealed {
        fn erased_write_output(&mut self, from: &[u8]) -> Result<(), HostError>;
        fn erased_write_debug(&self, msg: &str);
        fn erased_read_input(&mut self) -> Result<Option<input::Message>, HostError>;
        fn erased_store_has(
            &self,
            path: erase::Path<'_>,
        ) -> Result<Option<ValueType>, HostError>;
        fn erased_store_read(
            &self,
            path: erase::Path<'_>,
            from_offset: usize,
            max_bytes: usize,
        ) -> Result<Vec<u8>, HostError>;
        fn erased_store_read_slice(
            &self,
            path: erase::Path<'_>,
            from_offset: usize,
            buffer: &mut [u8],
        ) -> Result<usize, HostError>;
        fn erased_store_read_all(
            &self,
            path: erase::Path<'_>,
        ) -> Result<Vec<u8>, HostError>;
        fn erased_store_write(
            &mut self,
            path: erase::Path<'_>,
            src: &[u8],
            at_offset: usize,
        ) -> Result<(), HostError>;
        fn erased_store_write_all(
            &mut self,
            path: erase::Path<'_>,
            src: &[u8],
        ) -> Result<(), HostError>;
        fn erased_store_delete(&mut self, path: erase::Path<'_>)
            -> Result<(), HostError>;
        fn erased_store_delete_value(
            &mut self,
            path: erase::Path<'_>,
        ) -> Result<(), HostError>;
        fn erased_store_count_subkeys(
            &self,
            prefix: erase::Path<'_>,
        ) -> Result<u64, HostError>;
        fn erased_store_move(
            &mut self,
            from_path: erase::Path<'_>,
            to_path: erase::Path<'_>,
        ) -> Result<(), HostError>;
        fn erased_store_copy(
            &mut self,
            from_path: erase::Path<'_>,
            to_path: erase::Path<'_>,
        ) -> Result<(), HostError>;
        fn erased_reveal_preimage(
            &self,
            hash: &[u8; 33],
            destination: &mut [u8],
        ) -> Result<usize, RevealError>;
        fn erased_store_value_size(
            &self,
            path: erase::Path<'_>,
        ) -> Result<usize, HostError>;
        fn erased_mark_for_reboot(&mut self) -> Result<(), HostError>;
        fn erased_reveal_metadata(&self) -> RollupMetadata;
        fn erased_reveal_dal_page(
            &self,
            published_level: i32,
            slot_index: u8,
            page_index: i16,
            destination: &mut [u8],
        ) -> Result<usize, HostError>;
        fn erased_reveal_dal_parameters(&self) -> RollupDalParameters;
        fn erased_last_run_aborted(&self) -> Result<bool, HostError>;
        fn erased_upgrade_failed(&self) -> Result<bool, HostError>;
        fn erased_restart_forced(&self) -> Result<bool, HostError>;
        fn erased_reboot_left(&self) -> Result<u32, HostError>;
        fn erased_runtime_version(&self) -> Result<String, HostError>;
    }

    mod erase {
        use super::*;

        /// Wrapping `path::Path` permits a sized value to be passed
        /// to methods of `Runtime`.
        #[derive(Debug, Display)]
        pub struct Path<'a>(pub(crate) &'a dyn path::Path);
    }

    unsafe impl<'a> path::Path for erase::Path<'a> {
        fn as_bytes(&self) -> &[u8] {
            self.0.as_bytes()
        }
    }

    impl<T> sealed::runtime::Sealed for T where T: HostRuntime {}

    impl<T> Runtime for T
    where
        T: HostRuntime,
    {
        fn erased_write_output(&mut self, from: &[u8]) -> Result<(), HostError> {
            self.write_output(from)
        }

        fn erased_write_debug(&self, msg: &str) {
            self.write_debug(msg)
        }

        fn erased_read_input(&mut self) -> Result<Option<input::Message>, HostError> {
            self.read_input()
        }

        fn erased_store_has(
            &self,
            path: erase::Path<'_>,
        ) -> Result<Option<ValueType>, HostError> {
            self.store_has(&path)
        }

        fn erased_store_read(
            &self,
            path: erase::Path<'_>,
            from_offset: usize,
            max_bytes: usize,
        ) -> Result<Vec<u8>, HostError> {
            self.store_read(&path, from_offset, max_bytes)
        }

        fn erased_store_read_slice(
            &self,
            path: erase::Path<'_>,
            from_offset: usize,
            buffer: &mut [u8],
        ) -> Result<usize, HostError> {
            self.store_read_slice(&path, from_offset, buffer)
        }

        fn erased_store_read_all(
            &self,
            path: erase::Path<'_>,
        ) -> Result<Vec<u8>, HostError> {
            self.store_read_all(&path)
        }

        fn erased_store_write(
            &mut self,
            path: erase::Path<'_>,
            src: &[u8],
            at_offset: usize,
        ) -> Result<(), HostError> {
            self.store_write(&path, src, at_offset)
        }

        fn erased_store_write_all(
            &mut self,
            path: erase::Path<'_>,
            src: &[u8],
        ) -> Result<(), HostError> {
            self.store_write_all(&path, src)
        }

        fn erased_store_delete(
            &mut self,
            path: erase::Path<'_>,
        ) -> Result<(), HostError> {
            self.store_delete(&path)
        }

        fn erased_store_delete_value(
            &mut self,
            path: erase::Path<'_>,
        ) -> Result<(), HostError> {
            self.store_delete_value(&path)
        }

        fn erased_store_count_subkeys(
            &self,
            prefix: erase::Path<'_>,
        ) -> Result<u64, HostError> {
            self.store_count_subkeys(&prefix)
        }

        fn erased_store_move(
            &mut self,
            from_path: erase::Path<'_>,
            to_path: erase::Path<'_>,
        ) -> Result<(), HostError> {
            self.store_move(&from_path, &to_path)
        }

        fn erased_store_copy(
            &mut self,
            from_path: erase::Path<'_>,
            to_path: erase::Path<'_>,
        ) -> Result<(), HostError> {
            self.store_copy(&from_path, &to_path)
        }

        fn erased_reveal_preimage(
            &self,
            hash: &[u8; 33],
            destination: &mut [u8],
        ) -> Result<usize, RevealError> {
            self.reveal_preimage(hash, destination)
        }

        fn erased_store_value_size(
            &self,
            path: erase::Path<'_>,
        ) -> Result<usize, HostError> {
            self.store_value_size(&path)
        }

        fn erased_mark_for_reboot(&mut self) -> Result<(), HostError> {
            self.mark_for_reboot()
        }

        fn erased_reveal_metadata(&self) -> RollupMetadata {
            self.reveal_metadata()
        }

        fn erased_reveal_dal_page(
            &self,
            published_level: i32,
            slot_index: u8,
            page_index: i16,
            destination: &mut [u8],
        ) -> Result<usize, HostError> {
            self.reveal_dal_page(published_level, slot_index, page_index, destination)
        }

        fn erased_reveal_dal_parameters(&self) -> RollupDalParameters {
            self.reveal_dal_parameters()
        }

        fn erased_last_run_aborted(&self) -> Result<bool, HostError> {
            self.last_run_aborted()
        }

        fn erased_upgrade_failed(&self) -> Result<bool, HostError> {
            self.upgrade_failed()
        }

        fn erased_restart_forced(&self) -> Result<bool, HostError> {
            self.restart_forced()
        }

        fn erased_reboot_left(&self) -> Result<u32, HostError> {
            self.reboot_left()
        }

        fn erased_runtime_version(&self) -> Result<String, HostError> {
            self.runtime_version()
        }
    }

    impl HostRuntime for dyn Runtime {
        fn write_output(&mut self, from: &[u8]) -> Result<(), HostError> {
            self.erased_write_output(from)
        }

        fn write_debug(&self, msg: &str) {
            self.erased_write_debug(msg)
        }

        fn read_input(&mut self) -> Result<Option<input::Message>, HostError> {
            self.erased_read_input()
        }

        fn store_has<T: Path>(&self, path: &T) -> Result<Option<ValueType>, HostError> {
            self.erased_store_has(erase::Path(path))
        }

        fn store_read<T: Path>(
            &self,
            path: &T,
            from_offset: usize,
            max_bytes: usize,
        ) -> Result<Vec<u8>, HostError> {
            self.erased_store_read(erase::Path(path), from_offset, max_bytes)
        }

        fn store_read_slice<T: Path>(
            &self,
            path: &T,
            from_offset: usize,
            buffer: &mut [u8],
        ) -> Result<usize, HostError> {
            self.erased_store_read_slice(erase::Path(path), from_offset, buffer)
        }

        fn store_read_all(&self, path: &impl Path) -> Result<Vec<u8>, HostError> {
            self.erased_store_read_all(erase::Path(path))
        }

        fn store_write<T: Path>(
            &mut self,
            path: &T,
            src: &[u8],
            at_offset: usize,
        ) -> Result<(), HostError> {
            self.erased_store_write(erase::Path(path), src, at_offset)
        }

        fn store_write_all<T: Path>(
            &mut self,
            path: &T,
            src: &[u8],
        ) -> Result<(), HostError> {
            self.erased_store_write_all(erase::Path(path), src)
        }

        fn store_delete<T: Path>(&mut self, path: &T) -> Result<(), HostError> {
            self.erased_store_delete(erase::Path(path))
        }

        fn store_delete_value<T: Path>(&mut self, path: &T) -> Result<(), HostError> {
            self.erased_store_delete_value(erase::Path(path))
        }

        fn store_count_subkeys<T: Path>(&self, prefix: &T) -> Result<u64, HostError> {
            self.erased_store_count_subkeys(erase::Path(prefix))
        }

        fn store_move(
            &mut self,
            from_path: &impl Path,
            to_path: &impl Path,
        ) -> Result<(), HostError> {
            self.erased_store_move(erase::Path(from_path), erase::Path(to_path))
        }

        fn store_copy(
            &mut self,
            from_path: &impl Path,
            to_path: &impl Path,
        ) -> Result<(), HostError> {
            self.erased_store_copy(erase::Path(from_path), erase::Path(to_path))
        }

        fn reveal_preimage(
            &self,
            hash: &[u8; 33],
            destination: &mut [u8],
        ) -> Result<usize, RevealError> {
            self.erased_reveal_preimage(hash, destination)
        }

        fn store_value_size(&self, path: &impl Path) -> Result<usize, HostError> {
            self.erased_store_value_size(erase::Path(path))
        }

        fn mark_for_reboot(&mut self) -> Result<(), HostError> {
            self.erased_mark_for_reboot()
        }

        fn reveal_metadata(&self) -> RollupMetadata {
            self.erased_reveal_metadata()
        }

        fn reveal_dal_page(
            &self,
            published_level: i32,
            slot_index: u8,
            page_index: i16,
            destination: &mut [u8],
        ) -> Result<usize, HostError> {
            self.erased_reveal_dal_page(
                published_level,
                slot_index,
                page_index,
                destination,
            )
        }

        fn reveal_dal_parameters(&self) -> RollupDalParameters {
            self.erased_reveal_dal_parameters()
        }

        fn last_run_aborted(&self) -> Result<bool, HostError> {
            self.erased_last_run_aborted()
        }

        fn upgrade_failed(&self) -> Result<bool, HostError> {
            self.erased_upgrade_failed()
        }

        fn restart_forced(&self) -> Result<bool, HostError> {
            self.erased_restart_forced()
        }

        fn reboot_left(&self) -> Result<u32, HostError> {
            self.erased_reboot_left()
        }

        fn runtime_version(&self) -> Result<String, HostError> {
            self.erased_runtime_version()
        }
    }
}

pub struct JsHostRuntime<'a> {
    inner: &'a mut dyn erased_runtime::Runtime,
}

impl<'a> JsHostRuntime<'a> {
    pub fn new<R: Runtime>(rt: &'a mut R) -> JsHostRuntime<'static> {
        let rt_ptr: *mut dyn erased_runtime::Runtime = rt;

        // SAFETY
        // From the pov of the `Host` struct, it is permitted to cast
        // the `rt` reference to `'static` since the lifetime of `Host`
        // is always shorter than the lifetime of `rt`
        let rt: &'a mut dyn erased_runtime::Runtime = unsafe { &mut *rt_ptr };

        let jhr: Self = Self { inner: rt };

        unsafe { std::mem::transmute(jhr) }
    }
}

impl<'a: 'static> HostRuntime for JsHostRuntime<'a> {
    fn write_output(&mut self, from: &[u8]) -> Result<(), HostError> {
        self.inner.write_output(from)
    }

    fn write_debug(&self, msg: &str) {
        self.inner.write_debug(msg)
    }

    fn read_input(&mut self) -> Result<Option<input::Message>, HostError> {
        self.inner.read_input()
    }

    fn store_has<T: path::Path>(&self, path: &T) -> Result<Option<ValueType>, HostError> {
        self.inner.store_has(path)
    }

    fn store_read<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, HostError> {
        self.inner.store_read(path, from_offset, max_bytes)
    }

    fn store_read_slice<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        buffer: &mut [u8],
    ) -> Result<usize, HostError> {
        self.inner.store_read_slice(path, from_offset, buffer)
    }

    fn store_read_all(&self, path: &impl path::Path) -> Result<Vec<u8>, HostError> {
        self.inner.store_read_all(path)
    }

    fn store_write<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
        at_offset: usize,
    ) -> Result<(), HostError> {
        self.inner.store_write(path, src, at_offset)
    }

    fn store_write_all<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
    ) -> Result<(), HostError> {
        self.inner.store_write_all(path, src)
    }

    fn store_delete<T: path::Path>(&mut self, path: &T) -> Result<(), HostError> {
        self.inner.store_delete(path)
    }

    fn store_delete_value<T: path::Path>(&mut self, path: &T) -> Result<(), HostError> {
        self.inner.store_delete_value(path)
    }

    fn store_count_subkeys<T: path::Path>(&self, prefix: &T) -> Result<u64, HostError> {
        self.inner.store_count_subkeys(prefix)
    }

    fn store_move(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), HostError> {
        self.inner.store_move(from_path, to_path)
    }

    fn store_copy(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), HostError> {
        self.inner.store_copy(from_path, to_path)
    }

    fn reveal_preimage(
        &self,
        hash: &[u8; 33],
        destination: &mut [u8],
    ) -> Result<usize, RevealError> {
        self.inner.reveal_preimage(hash, destination)
    }

    fn store_value_size(&self, path: &impl path::Path) -> Result<usize, HostError> {
        self.inner.store_value_size(path)
    }

    fn mark_for_reboot(&mut self) -> Result<(), HostError> {
        self.inner.mark_for_reboot()
    }

    fn reveal_metadata(&self) -> RollupMetadata {
        self.inner.reveal_metadata()
    }

    fn reveal_dal_page(
        &self,
        published_level: i32,
        slot_index: u8,
        page_index: i16,
        destination: &mut [u8],
    ) -> Result<usize, HostError> {
        self.inner
            .reveal_dal_page(published_level, slot_index, page_index, destination)
    }

    fn reveal_dal_parameters(&self) -> RollupDalParameters {
        self.inner.reveal_dal_parameters()
    }

    fn last_run_aborted(&self) -> Result<bool, HostError> {
        self.inner.last_run_aborted()
    }

    fn upgrade_failed(&self) -> Result<bool, HostError> {
        self.inner.upgrade_failed()
    }

    fn restart_forced(&self) -> Result<bool, HostError> {
        self.inner.restart_forced()
    }

    fn reboot_left(&self) -> Result<u32, HostError> {
        self.inner.reboot_left()
    }

    fn runtime_version(&self) -> Result<String, HostError> {
        self.inner.runtime_version()
    }
}
