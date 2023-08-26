use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::num::NonZeroU32;

use boa_engine::object::{NativeObject, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{
    context::HostHooks, object::builtins::JsFunction, realm::Realm, Context,
    JsNativeError, JsResult,
};
use boa_gc::{empty_trace, Finalize, GcRef, GcRefCell, GcRefMut, Trace};
use derive_more::{Deref, DerefMut};
use getrandom::{register_custom_getrandom, Error as RandomError};
use tezos_smart_rollup_host::runtime::{Runtime, RuntimeError, ValueType};
use tezos_smart_rollup_host::{input, metadata, path};

pub type HostError = RuntimeError;

#[derive(Debug, Deref, DerefMut)]
pub struct Host<H: Runtime + 'static> {
    rt: &'static mut H,
}

impl<H> Finalize for Host<H> where H: Runtime + 'static {}

unsafe impl<H> Trace for Host<H>
where
    H: Runtime + 'static,
{
    empty_trace!();
}

impl<H> Host<H>
where
    H: Runtime,
{
    pub unsafe fn new(rt: &mut H) -> Self {
        let rt_ptr: *mut H = rt;

        // SAFETY
        // From the pov of the `Host` struct, it is permitted to cast
        // the `rt` reference to `'static` since the lifetime of `Host`
        // is always shorter than the lifetime of `rt`
        let rt: &'static mut H = &mut *rt_ptr;

        Self { rt }
    }
}

impl<H> Runtime for Host<H>
where
    H: Runtime,
{
    fn write_output(&mut self, from: &[u8]) -> Result<(), RuntimeError> {
        self.rt.write_output(from)
    }

    fn write_debug(&self, msg: &str) {
        self.rt.write_debug(msg)
    }

    fn read_input(&mut self) -> Result<Option<input::Message>, RuntimeError> {
        self.rt.read_input()
    }

    fn store_has<T: path::Path>(
        &self,
        path: &T,
    ) -> Result<Option<ValueType>, RuntimeError> {
        self.rt.store_has(path)
    }

    fn store_read<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.rt.store_read(path, from_offset, max_bytes)
    }

    fn store_read_slice<T: path::Path>(
        &self,
        path: &T,
        from_offset: usize,
        buffer: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        self.rt.store_read_slice(path, from_offset, buffer)
    }

    fn store_read_all(&self, path: &impl path::Path) -> Result<Vec<u8>, RuntimeError> {
        self.rt.store_read_all(path)
    }

    fn store_write<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
        at_offset: usize,
    ) -> Result<(), RuntimeError> {
        self.rt.store_write(path, src, at_offset)
    }

    fn store_write_all<T: path::Path>(
        &mut self,
        path: &T,
        src: &[u8],
    ) -> Result<(), RuntimeError> {
        self.rt.store_write_all(path, src)
    }

    fn store_delete<T: path::Path>(&mut self, path: &T) -> Result<(), RuntimeError> {
        self.rt.store_delete(path)
    }

    fn store_delete_value<T: path::Path>(
        &mut self,
        path: &T,
    ) -> Result<(), RuntimeError> {
        self.rt.store_delete_value(path)
    }

    fn store_count_subkeys<T: path::Path>(
        &self,
        prefix: &T,
    ) -> Result<u64, RuntimeError> {
        self.rt.store_count_subkeys(prefix)
    }

    fn store_move(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), RuntimeError> {
        self.rt.store_move(from_path, to_path)
    }

    fn store_copy(
        &mut self,
        from_path: &impl path::Path,
        to_path: &impl path::Path,
    ) -> Result<(), RuntimeError> {
        self.rt.store_copy(from_path, to_path)
    }

    fn reveal_preimage(
        &self,
        hash: &[u8; 33],
        destination: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        self.rt.reveal_preimage(hash, destination)
    }

    fn store_value_size(&self, path: &impl path::Path) -> Result<usize, RuntimeError> {
        self.rt.store_value_size(path)
    }

    fn mark_for_reboot(&mut self) -> Result<(), RuntimeError> {
        self.rt.mark_for_reboot()
    }

    fn reveal_metadata(&self) -> metadata::RollupMetadata {
        self.rt.reveal_metadata()
    }

    fn last_run_aborted(&self) -> Result<bool, RuntimeError> {
        self.rt.last_run_aborted()
    }

    fn upgrade_failed(&self) -> Result<bool, RuntimeError> {
        self.rt.upgrade_failed()
    }

    fn restart_forced(&self) -> Result<bool, RuntimeError> {
        self.rt.restart_forced()
    }

    fn reboot_left(&self) -> Result<u32, RuntimeError> {
        self.rt.reboot_left()
    }

    fn runtime_version(&self) -> Result<String, RuntimeError> {
        self.rt.runtime_version()
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

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = RandomError::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> std::result::Result<(), RandomError> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(RandomError::from(code))
}

register_custom_getrandom!(always_fail);

pub trait Api {
    /// Initialize a JSTZ runtime API
    fn init<H: Runtime + 'static>(self, context: &mut Context<'_>);
}

/// A newtype over [`TypeId`] that is traced
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Deref, DerefMut)]
pub struct TracedTypeId(pub TypeId);

impl TracedTypeId {
    pub fn of<T: Any + ?Sized>() -> Self {
        Self(TypeId::of::<T>())
    }
}

impl Finalize for TracedTypeId {}

unsafe impl Trace for TracedTypeId {
    empty_trace!();
}

/// Map used to store the host defined objects.
type HostDefinedMap = HashMap<TracedTypeId, GcRefCell<Box<dyn NativeObject>>>;

/// This represents the `ECMAScript` specification notion of 'host defined'
/// objects.
///
/// This allows storing types which are mapped by their [`TypeId`].
#[derive(Trace, Finalize)]
pub struct HostDefined {
    env: HostDefinedMap,
}

unsafe fn downcast_boxed_native_object_unchecked<T: NativeObject>(
    obj: Box<dyn NativeObject>,
) -> Box<T> {
    let raw: *mut dyn NativeObject = Box::into_raw(obj);
    Box::from_raw(raw as *mut T)
}

impl HostDefined {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
        }
    }

    #[track_caller]
    pub fn insert<T: NativeObject>(&mut self, value: T) -> Option<Box<T>> {
        self.env
            .insert(TracedTypeId::of::<T>(), GcRefCell::new(Box::new(value)))
            .map(|obj| unsafe {
                downcast_boxed_native_object_unchecked(obj.into_inner())
            })
    }

    #[track_caller]
    pub fn remove<T: NativeObject>(&mut self) -> Option<Box<T>> {
        self.env.remove(&TracedTypeId::of::<T>()).map(|obj| unsafe {
            downcast_boxed_native_object_unchecked(obj.into_inner())
        })
    }

    #[track_caller]
    pub fn has<T: NativeObject>(&self) -> bool {
        self.env.contains_key(&TracedTypeId::of::<T>())
    }

    #[track_caller]
    pub fn get<T: NativeObject>(&self) -> Option<GcRef<'_, T>> {
        let entry = self.env.get(&TracedTypeId::of::<T>())?;

        Some(GcRef::map(entry.borrow(), |obj| {
            obj.as_ref()
                .as_any()
                .downcast_ref::<T>()
                .expect("Why cruel world!")
        }))
    }

    #[track_caller]
    pub fn get_mut<T: NativeObject>(
        &self,
    ) -> Option<GcRefMut<'_, Box<dyn NativeObject>, T>> {
        let entry = self.env.get(&TracedTypeId::of::<T>())?;

        Some(GcRefMut::map(
            entry.borrow_mut(),
            |obj: &mut Box<dyn NativeObject>| {
                obj.as_mut()
                    .as_mut_any()
                    .downcast_mut::<T>()
                    .expect("Why cruel world!")
            },
        ))
    }

    #[track_caller]
    pub fn clear(&mut self) {
        self.env.clear();
    }
}

#[macro_export]
macro_rules! host_defined {
    ($context:expr, $host_defined:ident) => {
        let host_defined_binding = $context
            .global_object()
            .get(jstz_core::host::HostDefined::NAME, $context)
            .expect(&format!(
                "{:?} should be defined",
                jstz_core::host::HostDefined::NAME
            ));

        let $host_defined = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<host::HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");
    };
    ($context:expr, mut $host_defined:ident) => {
        let host_defined_binding = $context
            .global_object()
            .get(host::HostDefined::NAME, $context)
            .expect(&format!("{:?} should be defined", host::HostDefined::NAME));

        let mut $host_defined = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<host::HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");
    };
}

// HostDefined is internally an API
// TODO: HostDefined should be added to `Realm` as a patch to boa_engine

impl HostDefined {
    pub const NAME: &'static str = "__JSTZ__HOSTDEFINED";
}

impl Api for HostDefined {
    fn init<H: Runtime + 'static>(self, context: &mut Context<'_>) {
        let host_defined = ObjectInitializer::with_native(self, context).build();

        context
            .register_global_property(Self::NAME, host_defined, Attribute::all())
            .expect(&format!(
                "{:?} object should only be defined once",
                Self::NAME
            ))
    }
}
