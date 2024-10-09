use std::marker::PhantomData;

use boa_engine::{
    builtins::object::OrdinaryObject,
    context::intrinsics::StandardConstructor,
    js_string,
    object::{
        builtins::JsFunction, ConstructorBuilder, ErasedObject, FunctionBinding,
        FunctionObjectBuilder, JsPrototype, PROTOTYPE,
    },
    property::{Attribute, PropertyDescriptor, PropertyKey},
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
};
use boa_gc::{Finalize, GcRef, GcRefMut, Trace};

pub use boa_engine::{object::NativeObject, NativeFunction};

use crate::value::IntoJs;

/// This struct permits Rust types to be passed around as JavaScript objects.
#[derive(Trace, Finalize, JsData, Debug)]
pub struct JsNativeObject<T: NativeObject> {
    inner: JsValue,
    _phantom: PhantomData<T>,
}

impl<T: NativeObject> Clone for JsNativeObject<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: self._phantom,
        }
    }
}

impl<T: NativeObject> JsNativeObject<T> {
    pub fn is(value: &JsValue) -> bool {
        value.as_object().map_or(false, JsObject::is::<T>)
    }

    pub fn new_with_proto<C, P>(
        prototype: P,
        native_object: T,
        context: &mut Context,
    ) -> JsResult<Self>
    where
        C: NativeClass<Instance = T>,
        P: Into<Option<JsObject>>,
    {
        let class = context.global_object().get(js_string!(C::NAME), context)?;
        let JsValue::Object(ref class_constructor) = class else {
            return Err(JsNativeError::typ()
                .with_message(format!(
                    "invalid constructor for native class `{}` ",
                    C::NAME
                ))
                .into());
        };

        let JsValue::Object(class_prototype) =
            class_constructor.get(PROTOTYPE, context)?
        else {
            return Err(JsNativeError::typ()
                .with_message(format!(
                    "invalid default prototype for native class `{}`",
                    C::NAME
                ))
                .into());
        };

        let prototype =
            <P as Into<Option<JsObject>>>::into(prototype).unwrap_or(class_prototype);

        let obj = JsObject::from_proto_and_data(prototype, native_object);

        Ok(Self {
            inner: obj.into(),
            _phantom: PhantomData,
        })
    }

    pub fn new<C>(native_object: T, context: &mut Context) -> JsResult<Self>
    where
        C: NativeClass<Instance = T>,
    {
        Self::new_with_proto::<C, _>(None, native_object, context)
    }

    pub fn inner(&self) -> &JsValue {
        &self.inner
    }

    pub fn to_inner(&self) -> JsValue {
        self.inner.clone()
    }

    pub fn object(&self) -> &JsObject {
        self.inner.as_object().expect("Expected `JsObject`")
    }

    pub fn to_object(&self) -> JsObject {
        self.object().clone()
    }

    pub fn deref(&self) -> GcRef<'_, T> {
        self.object()
            .downcast_ref::<T>()
            .expect("Type mismatch in `JsNativeObject`")
    }

    pub fn deref_mut(&self) -> GcRefMut<'_, ErasedObject, T> {
        self.object()
            .downcast_mut::<T>()
            .expect("Type mismatch in `JsNativeObject`")
    }
}

impl<T: NativeObject> From<JsNativeObject<T>> for JsValue {
    fn from(val: JsNativeObject<T>) -> Self {
        val.to_inner()
    }
}

impl<T: NativeObject> IntoJs for JsNativeObject<T> {
    #[inline]
    fn into_js(self, _context: &mut Context) -> JsValue {
        self.into()
    }
}

impl<T: NativeObject> TryFrom<JsValue> for JsNativeObject<T> {
    type Error = JsError;

    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        if !Self::is(&value) {
            return Err(JsNativeError::typ()
                .with_message("Type mismatch in `JsNativeObject`")
                .into());
        }

        Ok(Self {
            inner: value,
            _phantom: PhantomData,
        })
    }
}

pub struct Accessor {
    pub name: &'static str,
    pub get: Option<JsFunction>,
    pub set: Option<JsFunction>,
}

impl Accessor {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            get: None,
            set: None,
        }
    }

    pub fn get(mut self, function: NativeFunction, context: &mut Context) -> Self {
        let get = FunctionObjectBuilder::new(context.realm(), function)
            .name(self.name)
            .length(0)
            .build();
        self.get = Some(get);
        self
    }

    pub fn set(mut self, function: NativeFunction, context: &mut Context) -> Self {
        let set = FunctionObjectBuilder::new(context.realm(), function)
            .name(format!("set_{}", self.name))
            .length(1)
            .build();
        self.set = Some(set);
        self
    }
}

#[macro_export]
macro_rules! accessor {
    ($context:expr, $instance:ident, $name:expr, get:(($gthis:ident, $gcontext:ident) => $get:expr) $(, set:(($sthis:ident, $sarg:ident : $sarg_ty:ty, $scontext:ident) => $set:expr) )?) => {
        $crate::native::Accessor::new($name)
            .get(
                boa_engine::NativeFunction::from_fn_ptr(|this, _args, $gcontext| {
                    let $gthis = $instance::try_from_js(this)?;

                    $get
                }),
                $context,
            )
            $(
                .set(
                    boa_engine::NativeFunction::from_fn_ptr(|this, args, $scontext| {
                        let mut $sthis = $instance::try_from_js(this)?;
                        let $sarg: $sarg_ty =
                            args.get_or_undefined(0).try_js_into($scontext)?;

                        $set;

                        Ok(boa_engine::JsValue::null())
                    }),
                    $context,
                )
            )?
    };
}

/// Class builder which allows adding methods and static methods to the class.
#[derive(Debug)]
pub struct ClassBuilder<'ctx> {
    builder: ConstructorBuilder<'ctx>,
}

impl<'ctx> ClassBuilder<'ctx> {
    fn new<T>(context: &'ctx mut Context) -> Self
    where
        T: NativeClass,
    {
        let mut builder = ConstructorBuilder::new(
            context,
            NativeFunction::from_fn_ptr(raw_constructor::<T>),
        );
        builder.name(T::NAME);
        builder.length(T::LENGTH);
        Self { builder }
    }

    fn build(self) -> StandardConstructor {
        self.builder.build()
    }

    /// Add a method to the class.
    ///
    /// It is added to `prototype`.
    pub fn method<N>(
        &mut self,
        name: N,
        length: usize,
        function: NativeFunction,
    ) -> &mut Self
    where
        N: Into<FunctionBinding>,
    {
        self.builder.method(function, name, length);
        self
    }

    /// Add a method to the prototype but with enumerable: true
    // It appears to be impossible to keep same interface of
    // `method<N>`, because FunctionBinding .name field is pub(crate)?
    pub fn enumerable_method(
        &mut self,
        name: JsString,
        length: usize,
        function: NativeFunction,
    ) -> &mut Self {
        let context = self.builder.context();
        let function = FunctionObjectBuilder::new(context.realm(), function)
            .name(name.clone())
            .length(length)
            .constructor(false)
            .build();
        let mut attribute: Attribute = Attribute::default();
        attribute.set_writable(true);
        attribute.set_enumerable(true);
        attribute.set_configurable(true);
        self.builder.property::<JsString, JsValue>(
            name.clone(),
            function.into(),
            attribute,
        );
        self
    }

    /// Add a static method to the class.
    ///
    /// It is added to class object itself.
    pub fn static_method<N>(
        &mut self,
        name: N,
        length: usize,
        function: NativeFunction,
    ) -> &mut Self
    where
        N: Into<FunctionBinding>,
    {
        self.builder.static_method(function, name, length);
        self
    }

    /// Add a data property to the class, with the specified attribute.
    ///
    /// It is added to `prototype`.
    pub fn property<K, V>(&mut self, key: K, value: V, attribute: Attribute) -> &mut Self
    where
        K: Into<PropertyKey>,
        V: Into<JsValue>,
    {
        self.builder.property(key, value, attribute);
        self
    }

    /// Add a static data property to the class, with the specified attribute.
    ///
    /// It is added to class object itself.
    pub fn static_property<K, V>(
        &mut self,
        key: K,
        value: V,
        attribute: Attribute,
    ) -> &mut Self
    where
        K: Into<PropertyKey>,
        V: Into<JsValue>,
    {
        self.builder.static_property(key, value, attribute);
        self
    }

    /// Add an accessor property to the class, with the specified attribute.
    ///
    /// It is added to `prototype`.
    pub fn accessor<K>(
        &mut self,
        key: K,
        accessor: Accessor,
        attribute: Attribute,
    ) -> &mut Self
    where
        K: Into<PropertyKey>,
    {
        self.builder
            .accessor(key, accessor.get, accessor.set, attribute);
        self
    }

    /// Add a static accessor property to the class, with the specified attribute.
    ///
    /// It is added to class object itself.
    pub fn static_accessor<K>(
        &mut self,
        key: K,
        accessor: Accessor,
        attribute: Attribute,
    ) -> &mut Self
    where
        K: Into<PropertyKey>,
    {
        self.builder
            .static_accessor(key, accessor.get, accessor.set, attribute);
        self
    }

    /// Add a property descriptor to the class, with the specified attribute.
    ///
    /// It is added to `prototype`.
    pub fn property_descriptor<K, P>(&mut self, key: K, property: P) -> &mut Self
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        self.builder.property_descriptor(key, property);
        self
    }

    /// Add a static property descriptor to the class, with the specified attribute.
    ///
    /// It is added to class object itself.
    pub fn static_property_descriptor<K, P>(&mut self, key: K, property: P) -> &mut Self
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        self.builder.static_property_descriptor(key, property);
        self
    }

    /// Specify the parent prototype for the class
    pub fn inherit<O: Into<JsPrototype>>(&mut self, prototype: O) -> &mut Self {
        self.builder.inherit(prototype);
        self
    }

    /// Return the current context.
    #[inline]
    pub fn context(&mut self) -> &mut Context {
        self.builder.context()
    }
}

fn raw_constructor<T: NativeClass>(
    target: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    if target.is_undefined() {
        return Err(JsNativeError::typ()
            .with_message(format!(
                "cannot call constructor of native class `{}` without new",
                T::NAME
            ))
            .into());
    }

    let prototype = target
        .as_object()
        .map(|obj| {
            obj.get(PROTOTYPE, context)
                .map(|val| val.as_object().cloned())
        })
        .transpose()?
        .flatten();

    let native_object = T::data_constructor(target, args, context)?;

    let object =
        JsNativeObject::new_with_proto::<T, _>(prototype, native_object, context)?;

    T::object_constructor(&object, args, context)?;

    Ok(object.inner.clone())
}

pub trait JsNativeObjectToString: NativeObject + Sized {
    fn to_string(this: &JsNativeObject<Self>, context: &mut Context)
        -> JsResult<JsValue>;
}

pub trait NativeClass {
    /// The Rust type of the class's instances.
    type Instance: NativeObject + Sized;

    /// The binding name of the class.
    const NAME: &'static str;

    /// The amount of arguments the class `constructor` takes, default is `0`.
    const LENGTH: usize = 0usize;

    /// The attributes the class will be bound with, default is `writable`, `enumerable`, `configurable`.
    const ATTRIBUTES: Attribute = Attribute::all();

    /// Creates the internal data for an instance of this class.
    fn data_constructor(
        target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<Self::Instance>;

    /// Initializes the properties of the constructed object for an instance of this class.
    ///
    /// Useful for initial additional properties in the constructed object that aren't part of the
    /// Rust internal data value that rely on the `this` or `args` -- e.g. when you need to create
    /// a cycle between native objects. For example usage, see `jstz_api::url::Url::object_constructor`.
    fn object_constructor(
        _this: &JsNativeObject<Self::Instance>,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<()> {
        Ok(())
    }

    /// Initializes the internals and the methods of the class.
    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()>;

    fn to_string(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue>
    where
        Self::Instance: JsNativeObjectToString,
    {
        if let Ok(native_obj) = JsNativeObject::<Self::Instance>::try_from(this.clone()) {
            Self::Instance::to_string(&native_obj, context)
        } else {
            OrdinaryObject::to_string(this, &[], context)
        }
    }
}

pub fn register_global_class<T: NativeClass>(context: &mut Context) -> JsResult<()> {
    let mut class_builder = ClassBuilder::new::<T>(context);
    T::init(&mut class_builder)?;

    let class = class_builder.build();
    let property = PropertyDescriptor::builder()
        .value(class.constructor())
        .writable(T::ATTRIBUTES.writable())
        .enumerable(T::ATTRIBUTES.enumerable())
        .configurable(T::ATTRIBUTES.configurable());

    context.global_object().define_property_or_throw(
        js_string!(T::NAME),
        property,
        context,
    )?;

    Ok(())
}
