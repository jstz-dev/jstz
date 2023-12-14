use std::{
    any::{Any, TypeId},
    collections::HashMap,
    io::Read,
    ops::{Deref, DerefMut},
};

pub use boa_engine::realm;
use boa_engine::{
    js_string,
    object::{builtins::JsPromise, NativeObject, ObjectInitializer},
    property::Attribute,
    Context, JsObject, JsResult, JsValue, Source,
};
use boa_gc::{empty_trace, Finalize, GcRef, GcRefCell, GcRefMut, Trace};
use derive_more::{Deref, DerefMut, From};

use crate::{
    native::{register_global_class, NativeClass},
    Api,
};

/// A newtype wrapper over `Module` tha maintains a reference
/// to the module's realm
#[derive(Debug, PartialEq, Eq, Clone, Trace, Finalize)]
pub struct Module {
    inner: boa_engine::Module,
    realm: Realm,
}

impl Deref for Module {
    type Target = boa_engine::Module;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Module {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Module {
    pub fn parse<R: Read>(
        src: Source<'_, R>,
        realm: Option<Realm>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let realm = realm.unwrap_or_else(|| context.realm().clone().into());

        let module = boa_engine::Module::parse(src, Some(realm.inner_realm()), context)?;
        Ok(Self {
            realm,
            inner: module,
        })
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }
}

/// A context handle is a local context with a
pub struct ContextHandle<'host, 's> {
    outer: realm::Realm,
    context: &'s mut Context<'host>,
}

impl<'host, 's> Deref for ContextHandle<'host, 's> {
    type Target = Context<'host>;

    fn deref(&self) -> &Self::Target {
        self.context
    }
}

impl<'host, 's> DerefMut for ContextHandle<'host, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.context
    }
}

impl<'host, 's> Drop for ContextHandle<'host, 's> {
    fn drop(&mut self) {
        self.context.enter_realm(self.outer.clone());
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Trace, Finalize, Deref, DerefMut, From)]
pub struct Realm {
    pub inner: realm::Realm,
}

impl Realm {
    pub fn inner_realm(&self) -> realm::Realm {
        self.deref().clone()
    }

    pub fn context_handle<'s, 'host>(
        &self,
        context: &'s mut Context<'host>,
    ) -> ContextHandle<'host, 's> {
        let outer = context.enter_realm(self.inner.clone());
        ContextHandle { outer, context }
    }

    pub fn global_object(&self, context: &mut Context<'_>) -> JsObject {
        self.context_handle(context).global_object()
    }

    pub fn register_global_class<T: NativeClass>(
        &self,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        let context = &mut self.context_handle(context);
        register_global_class::<T>(context)
    }

    pub fn register_api<T: Api>(&self, api: T, context: &mut Context<'_>) {
        let context = &mut self.context_handle(context);
        api.init(context)
    }

    /// Parses, compiles and evaluates the script `src`.
    pub fn eval<R: Read>(
        &self,
        src: Source<'_, R>,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        self.context_handle(context).eval(src)
    }

    /// Loads, links and evaluates a module.
    ///
    /// Returns the module instance and the module promise. Implementors must manually
    /// call `Runtime::run_event_loop` or poll/resolve the promise to drive the
    /// module's evaluation.  
    ///
    /// # Note
    ///
    /// This doesn't evaluate the module with the _module's_ realm, but the realm given
    /// as `self`.
    pub fn eval_module(
        &self,
        module: &Module,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> {
        let context = &mut self.context_handle(context);

        module.load_link_evaluate(context)
    }
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
#[derive(Trace, Finalize, Default)]
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
        Self::default()
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
            .get(
                ::boa_engine::js_string!($crate::realm::HostDefined::NAME),
                $context,
            )
            .expect(&format!(
                "{:?} should be defined",
                $crate::realm::HostDefined::NAME
            ));

        let $host_defined = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<$crate::realm::HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");
    };
    ($context:expr, mut $host_defined:ident) => {
        let host_defined_binding = $context
            .global_object()
            .get(
                ::boa_engine::js_string!($crate::realm::HostDefined::NAME),
                $context,
            )
            .expect(&format!(
                "{:?} should be defined",
                $crate::realm::HostDefined::NAME
            ));

        let mut $host_defined = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<$crate::realm::HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");
    };
}

impl HostDefined {
    pub const NAME: &'static str = "#JSTZ__HOSTDEFINED";

    pub(crate) fn init(self, context: &mut Context<'_>) {
        let host_defined = ObjectInitializer::with_native(self, context).build();

        context
            .register_global_property(
                js_string!(Self::NAME),
                host_defined,
                Attribute::all(),
            )
            .unwrap_or_else(|_| {
                panic!("{:?} object should only be defined once", Self::NAME)
            })
    }
}

impl Realm {
    pub fn new(context: &mut Context<'_>) -> JsResult<Self> {
        // 1. Create `boa_engine` realm with defined host hooks
        let realm = Self {
            inner: context.create_realm()?,
        };

        // 2. Initialize `HostDefined`
        {
            let mut context = realm.context_handle(context);
            HostDefined::new().init(&mut context);
        }

        Ok(realm)
    }
}
