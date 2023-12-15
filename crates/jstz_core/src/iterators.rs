//! Helpers for implementing iterables on boa.
//!
//! Currently this module provides helpers only for pair iterators,
//! i.e. for interfaces which have a "list of value pairs to iterate
//! over."
//!
//! More information:
//!  - [MDN documentation - Iteration protocols][mdn]
//!  - [WHATWG Web IDL specification - Iterable declarations (IDL)][idl]
//!  - [WHATWG Web IDL specification - Iterable declarations (ECMAScript binding)][es]
//!
//! # Usage example
//!
//! Implement [`PairIterable`] for your iterable type:
//! ```ignore
//! impl PairIterable for Foo {
//!     fn pair_iterable_len(&self) -> usize {
//!         // ...
//!     }
//!
//!     fn pair_iterable_get(
//!         &self,
//!         index: usize,
//!         context: &mut Context<'_>,
//!     ) -> JsResult<PairValue> {
//!         // ...
//!         Ok(PairValue { key, value })
//!     }
//! }
//! ```
//!
//! Define a new iterator class and implement [`PairIteratorClass`]:
//! ```ignore
//! struct FooIteratorClass;
//! impl PairIteratorClass for FooIteratorClass {
//!     type Iterable = Foo;
//!     const NAME: &'static str = "Foo Iterator";
//! }
//! ```
//! The iterator class will then automatically implement `NativeClass`.
//!
//! Register the iterator class in your API as usual:
//! ```ignore
//! impl jstz_core::Api for FooApi {
//!     // ....
//!     register_global_class::<FooIteratorClass>(context)
//!         .expect("The `Foo Iterator` class shouldn't exist yet");
//! }
//! ```
//!
//! In your iterable NativeClass, use
//! [`PairIterableMethods::define_pair_iterable_methods`]
//! to define the iterable methods (`@@iterator`, `entries`, `keys`,
//! `values`, `forEach`.)
//! ```ignore
//! impl NativeClass for FooClass {
//!     // ...
//!     fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
//!         // ...
//!         PairIterableMethods::<FooIteratorClass>::define_pair_iterable_methods(class)?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols
//! [idl]: https://webidl.spec.whatwg.org/#idl-iterable
//! [es]: https://webidl.spec.whatwg.org/#es-iterable

// FIXME: Fix doc tests

use std::marker::PhantomData;

use crate::{
    native::{ClassBuilder, JsNativeObject, NativeClass},
    value::IntoJs,
};
use boa_engine::{
    js_string,
    object::{builtins::JsArray, NativeObject, Object},
    value::TryFromJs,
    Context, JsError, JsNativeError, JsObject, JsResult, JsSymbol, JsValue,
    NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};

enum PairIteratorKind {
    KeyPlusValue,
    Key,
    Value,
}

impl TryFromJs for PairIteratorKind {
    fn try_from_js(value: &JsValue, _context: &mut Context<'_>) -> JsResult<Self> {
        let kind_str = value
            .as_string()
            .ok_or::<JsError>(
                JsNativeError::typ()
                    .with_message("expected string kind arg to pair iterator constructor")
                    .into(),
            )?
            .to_std_string()
            .map_err::<JsError, _>(|_| {
                JsNativeError::typ()
                    .with_message("invalid string kind arg to pair iterator constructor")
                    .into()
            })?;
        match kind_str.as_str() {
            "key+value" => Ok(PairIteratorKind::KeyPlusValue),
            "key" => Ok(PairIteratorKind::Key),
            "value" => Ok(PairIteratorKind::Value),
            &_ => Err(JsNativeError::typ()
                .with_message("unexpected string kind arg to pair iterator constructor")
                .into()),
        }
    }
}

/// Struct for pair iterable items, as returned by
/// [`PairIterable::pair_iterable_get`].
pub struct PairValue {
    pub key: JsValue,
    pub value: JsValue,
}

impl IntoJs for PairValue {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        JsArray::from_iter([self.key, self.value], context).into()
    }
}

/// Trait for pair iterable objects (objects which have a "list of
/// value pairs to iterate over.")
pub trait PairIterable: NativeObject {
    // I don't know how to disambiguate these without giving them
    // unique names
    /// Length of the list of value pairs to iterate over.
    fn pair_iterable_len(&self) -> JsResult<usize>;
    /// Get one of the value pairs. Should return `Ok` if the provided
    /// `index` is less than the current
    /// [`pair_iterable_len`][`PairIterable::pair_iterable_len`].
    fn pair_iterable_get(
        &self,
        index: usize,
        context: &mut Context<'_>,
    ) -> JsResult<PairValue>;
}

/// Rust type used for pair iterator objects. Not relevant to users.
pub struct PairIterator<T: PairIterable> {
    target: JsNativeObject<T>,
    kind: PairIteratorKind,
    index: usize,
}

impl<T: PairIterable> PairIterator<T> {
    pub fn entries(target: JsNativeObject<T>) -> PairIterator<T> {
        let kind = PairIteratorKind::KeyPlusValue;
        let index = 0;
        PairIterator {
            target,
            kind,
            index,
        }
    }

    pub fn values(target: JsNativeObject<T>) -> PairIterator<T> {
        let kind = PairIteratorKind::Value;
        let index = 0;
        PairIterator {
            target,
            kind,
            index,
        }
    }

    pub fn keys(target: JsNativeObject<T>) -> PairIterator<T> {
        let kind = PairIteratorKind::Key;
        let index = 0;
        PairIterator {
            target,
            kind,
            index,
        }
    }
}

impl<T: PairIterable> Finalize for PairIterator<T> {
    fn finalize(&self) {
        self.target.finalize();
    }
}

unsafe impl<T: PairIterable> Trace for PairIterator<T> {
    boa_gc::custom_trace!(this, {
        mark(&this.target);
    });
}

impl<T: PairIterable> PairIterator<T> {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `PairIterator`",
                    )
                    .into()
            })
    }
}

struct IteratorResult {
    done: bool,
    value: JsValue,
}

impl IntoJs for IteratorResult {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let obj = JsObject::with_object_proto(context.intrinsics());
        obj.create_data_property_or_throw(js_string!("value"), self.value, context)
            .expect("unexpected error while converting IteratorResult to JsValue");
        obj.create_data_property_or_throw(
            js_string!("done"),
            JsValue::Boolean(self.done),
            context,
        )
        .expect("unexpected error while converting IteratorResult to JsValue");
        obj.into()
    }
}

struct PairIteratorMethods<T: PairIterable> {
    _phantom: PhantomData<T>,
}

impl<T: PairIterable> PairIteratorMethods<T> {
    fn next(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut pair_iterator = PairIterator::<T>::try_from_js(this)?;
        if pair_iterator.index >= pair_iterator.target.deref().pair_iterable_len()? {
            let done = true;
            let value = JsValue::undefined();
            let result = IteratorResult { done, value };
            return Ok(result.into_js(context));
        }
        let pair = pair_iterator
            .target
            .deref()
            .pair_iterable_get(pair_iterator.index, context)?;
        pair_iterator.index += 1;
        let done = false;
        let value = match pair_iterator.kind {
            PairIteratorKind::KeyPlusValue => pair.into_js(context),
            PairIteratorKind::Key => pair.key,
            PairIteratorKind::Value => pair.value,
        };
        let result = IteratorResult { done, value };
        Ok(result.into_js(context))
    }
}

/// Trait for pair iterator classes.
///
/// Implementing this will automatically derive a [`NativeClass`]
/// implementation. See module docs for example.
pub trait PairIteratorClass {
    type Iterable: PairIterable;
    // might be nice to set NAME as <underlying class NAME> + "
    // Iterator" automatically
    const NAME: &'static str;
}

/// Provides
/// [`define_pair_iterable_methods`][PairIterableMethods::define_pair_iterable_methods]
/// helper.
pub struct PairIterableMethods<T: PairIteratorClass> {
    _phantom: PhantomData<T>,
}

impl<T: PairIteratorClass> PairIterableMethods<T> {
    /// Defines the pair iterable methods (`@@iterator`, `entries`,
    /// `keys`, `values`, `forEach`) on the [`ClassBuilder`] for an
    /// iterable object [`NativeClass`] impl. The type parameter is
    /// the [`PairIteratorClass`] which the user must define and
    /// register.
    ///
    /// # Example
    /// ```ignore
    /// impl NativeClass for FooClass {
    ///     // ...
    ///     fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
    ///         // ...
    ///         PairIterableMethods::<FooIteratorClass>::define_pair_iterable_methods(class)?;
    ///         Ok(())
    ///     }
    /// }
    /// ```
    pub fn define_pair_iterable_methods(
        class: &mut ClassBuilder<'_, '_>,
    ) -> JsResult<()> {
        // TODO workaround until JsSymbol::iterator() is pub
        let symbol_iterator: JsSymbol = class
            .context()
            .intrinsics()
            .constructors()
            .symbol()
            .constructor()
            .get(js_string!("iterator"), class.context())?
            .as_symbol()
            .ok_or(
                JsNativeError::typ().with_message("Symbol.iterator was not a Symbol?"),
            )?;

        class.method(
            symbol_iterator,
            0,
            NativeFunction::from_fn_ptr(
                |this: &JsValue,
                 _args: &[JsValue],
                 context: &mut Context<'_>|
                 -> JsResult<JsValue> {
                    let target = JsNativeObject::try_from(this.clone())?;
                    let pair_iterator = PairIterator::entries(target);
                    Ok(JsNativeObject::new::<T>(pair_iterator, context)?.to_inner())
                },
            ),
        );
        class.method(
            js_string!("entries"),
            0,
            NativeFunction::from_fn_ptr(
                |this: &JsValue,
                 _args: &[JsValue],
                 context: &mut Context<'_>|
                 -> JsResult<JsValue> {
                    let target = JsNativeObject::try_from(this.clone())?;
                    let pair_iterator = PairIterator::entries(target);
                    Ok(JsNativeObject::new::<T>(pair_iterator, context)?.to_inner())
                },
            ),
        );
        class.method(
            js_string!("keys"),
            0,
            NativeFunction::from_fn_ptr(
                |this: &JsValue,
                 _args: &[JsValue],
                 context: &mut Context<'_>|
                 -> JsResult<JsValue> {
                    let target = JsNativeObject::try_from(this.clone())?;
                    let pair_iterator = PairIterator::keys(target);
                    Ok(JsNativeObject::new::<T>(pair_iterator, context)?.to_inner())
                },
            ),
        );
        class.method(
            js_string!("values"),
            0,
            NativeFunction::from_fn_ptr(
                |this: &JsValue,
                 _args: &[JsValue],
                 context: &mut Context<'_>|
                 -> JsResult<JsValue> {
                    let target = JsNativeObject::try_from(this.clone())?;
                    let pair_iterator = PairIterator::values(target);
                    Ok(JsNativeObject::new::<T>(pair_iterator, context)?.to_inner())
                },
            ),
        );
        class.method(
            js_string!("forEach"),
            1,
            NativeFunction::from_fn_ptr(
                |this: &JsValue,
                 args: &[JsValue],
                 context: &mut Context<'_>|
                 -> JsResult<JsValue> {
                    let target: JsNativeObject<T::Iterable> =
                        JsNativeObject::try_from(this.clone())?;
                    let callback_arg = args.get(0).ok_or::<JsError>(
                        JsNativeError::typ()
                            .with_message("expected callback argument to forEach")
                            .into(),
                    )?;
                    // TODO is this the correct way to "convert" as in
                    // https://webidl.spec.whatwg.org/#dfn-convert-ecmascript-to-idl-value
                    // "to a Function"?
                    let callback = callback_arg.as_callable().ok_or::<JsError>(
                        JsNativeError::typ()
                            .with_message("forEach callback argument was not callable")
                            .into(),
                    )?;
                    let undef_this = JsValue::undefined();
                    let this_arg = args.get(1).unwrap_or(&undef_this);
                    let mut index = 0;
                    while index < target.deref().pair_iterable_len()? {
                        let pair = target.deref().pair_iterable_get(index, context)?;
                        let args = [pair.value, pair.key, target.to_inner()];
                        callback.call(this_arg, &args, context)?;
                        index += 1;
                    }
                    Ok(JsValue::undefined())
                },
            ),
        );

        Ok(())
    }
}

impl<T: PairIteratorClass> NativeClass for T {
    type Instance = PairIterator<T::Iterable>;
    const NAME: &'static str = T::NAME;

    // The constructor will take 2 args, corresponding to target and
    // kind. it's kind of weird that we even have to define a
    // constructor at all, since these objects should only be
    // instantiated by jstz
    const LENGTH: usize = 2;

    fn constructor(
        _this: &JsNativeObject<Self::Instance>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Self::Instance> {
        let init_arg = match args.get(0) {
            None => Err(JsError::from_native(
                JsNativeError::typ()
                    .with_message("expected 2 arguments to pair iterator constructor"),
            )),
            Some(init_arg) => Ok(init_arg),
        }?;
        let kind_arg = match args.get(1) {
            None => Err(JsError::from_native(
                JsNativeError::typ()
                    .with_message("expected 2 arguments to pair iterator constructor"),
            )),
            Some(kind_arg) => Ok(kind_arg),
        }?;

        let target = JsNativeObject::try_from(init_arg.clone())?;
        let kind = kind_arg.try_js_into(context)?;
        let index = 0;

        Ok(PairIterator {
            target,
            kind,
            index,
        })
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let iterator_prototype = class
            .context()
            .intrinsics()
            .objects()
            .iterator_prototypes()
            .iterator();
        class
            .method(
                js_string!("next"),
                0,
                NativeFunction::from_fn_ptr(PairIteratorMethods::<T::Iterable>::next),
            )
            .inherit(iterator_prototype);
        Ok(())
    }
}
