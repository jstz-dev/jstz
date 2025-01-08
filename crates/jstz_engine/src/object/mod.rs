use std::{marker::PhantomData, mem::MaybeUninit, pin::Pin, slice, sync::Arc};

use mozjs::{
    jsapi::{
        jsid, GetPropertyKeys, JSObject, JS_DefinePropertyById2, JS_DeletePropertyById,
        JS_GetPropertyById, JS_HasOwnPropertyById, JS_HasPropertyById, JS_NewPlainObject,
        JS_SetPropertyById,
    },
    rust::IdVector,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{AsRawHandle, AsRawHandleMut, AsRawPtr, GcPtr, Handle, HandleMut},
        Compartment, Finalize, Prolong, Trace,
    },
    letroot,
    value::JsValue,
};

mod property;

use property::{IntoPropertyKey, PropertyFlags, PropertyIteratorFlags, PropertyKey};

/// [`JsObject`] represents an ordinary object in the JavaScript engine.
///
/// More information:
///  - [MDN documentation](mdn)
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object
pub struct JsObject<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<*mut JSObject>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Finalize for JsObject<'a, C> {
    fn finalize(&self) {
        self.inner_ptr.finalize()
    }
}

unsafe impl<'a, C: Compartment> Trace for JsObject<'a, C> {
    custom_trace!(this, mark, {
        mark(&this.inner_ptr);
    });
}

impl<'a, C: Compartment> Clone for JsObject<'a, C> {
    fn clone(&self) -> Self {
        Self {
            inner_ptr: self.inner_ptr.clone(),
            marker: PhantomData,
        }
    }
}

impl<'a, C: Compartment> AsRawPtr for JsObject<'a, C> {
    type Ptr = *mut JSObject;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.inner_ptr.as_raw_ptr()
    }
}

impl<'a, C: Compartment> AsRawHandle for JsObject<'a, C> {
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr> {
        self.inner_ptr.as_raw_handle()
    }
}

impl<'a, C: Compartment> AsRawHandleMut for JsObject<'a, C> {
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr> {
        self.inner_ptr.as_raw_handle_mut()
    }
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for JsObject<'b, C> {
    type Aged = JsObject<'a, C>;
}

impl<'a, C: Compartment> JsObject<'a, C> {
    pub fn new<S>(cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        let inner_ptr = GcPtr::pinned(unsafe { JS_NewPlainObject(cx.as_raw_ptr()) });

        Self {
            inner_ptr,
            marker: PhantomData,
        }
    }

    /// Get property from object
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-get-o-p
    pub fn get<'cx, S, K>(
        &self,
        key: K,
        cx: &'cx mut Context<S>,
    ) -> Option<JsValue<'cx, C>>
    where
        'cx: 'a,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(pkey = key.into_key(cx)?; [cx]);
        letroot!(rval = JsValue::undefined(cx); [cx]);

        let res = unsafe {
            JS_GetPropertyById(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                pkey.handle(),
                rval.handle_mut(),
            )
        };

        if res {
            Some(rval.into_inner(cx))
        } else {
            None
        }
    }

    /// Set property of object
    ///
    /// Returns `false` if the property is read-only
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-set-o-p-v-throw

    // TODO: Implement `or_throw: bool` flag to handle when the property is read-only
    pub fn set<'b, 'cx, S, K>(
        &self,
        key: K,
        value: &JsValue<'b, C>,
        cx: &'cx mut Context<S>,
    ) -> Option<bool>
    where
        'cx: 'a,
        'cx: 'b,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(rooted_value = value.clone(); [cx]);
        letroot!(pkey = key.into_key(cx)?; [cx]);

        Some(unsafe {
            JS_SetPropertyById(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                pkey.handle(),
                rooted_value.handle(),
            )
        })
    }

    /// Check if object has property.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-hasproperty
    pub fn has_property<'cx, S, K>(&self, key: K, cx: &'cx mut Context<S>) -> bool
    where
        'cx: 'a,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(pkey = key.into_key(cx); [cx]);

        if let Some(pkey) = pkey.as_ref() {
            let mut found = false;
            if unsafe {
                JS_HasPropertyById(
                    cx.as_raw_ptr(),
                    rooted_self.handle(),
                    pkey.as_raw_handle(),
                    &mut found,
                )
            } {
                found
            } else {
                // TODO: clear pending exception
                false
            }
        } else {
            false
        }
    }

    /// Check if object has an own property.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-hasownproperty
    pub fn has_own_property<'cx, S, K>(&self, key: K, cx: &'cx mut Context<S>) -> bool
    where
        'cx: 'a,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(pkey = key.into_key(cx); [cx]);

        if let Some(pkey) = pkey.as_ref() {
            let mut found = false;
            if unsafe {
                JS_HasOwnPropertyById(
                    cx.as_raw_ptr(),
                    rooted_self.handle(),
                    pkey.as_raw_handle(),
                    &mut found,
                )
            } {
                found
            } else {
                // TODO: clear pending exception
                false
            }
        } else {
            false
        }
    }

    /// Define property.
    ///
    /// Returns `false` if the property was unable to be defined.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-definepropertyorthrow

    // TODO: Implement `or_throw: bool` flag
    pub fn define_property<'cx, 'b, S, K>(
        &self,
        key: K,
        value: &JsValue<'b, C>,
        attrs: PropertyFlags,
        cx: &'cx mut Context<S>,
    ) -> Option<bool>
    where
        'cx: 'b,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(rooted_value = value.clone(); [cx]);
        letroot!(pkey = key.into_key(cx)?; [cx]);

        Some(unsafe {
            JS_DefinePropertyById2(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                pkey.handle(),
                rooted_value.handle(),
                u32::from(attrs.bits()),
            )
        })
    }

    /// Deletes the property.
    ///
    /// Returns `false` if the element cannot be deleted.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-deletepropertyorthrow

    // TODO: Implement `or_throw: bool` flag
    pub fn delete<S, K>(&self, key: K, cx: &mut Context<S>) -> Option<bool>
    where
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(pkey = key.into_key(cx)?; [cx]);

        let mut result = MaybeUninit::uninit();
        Some(unsafe {
            JS_DeletePropertyById(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                pkey.handle(),
                result.as_mut_ptr(),
            )
        })
    }

    /// Returns an iterator over keys of the object.
    ///
    /// The keys are rooted until the iterator is dropped.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-getownpropertykeys
    pub fn keys<'b, S>(
        &self,
        flags: Option<PropertyIteratorFlags>,
        cx: &mut Context<S>,
    ) -> ObjectKeysIterator<'b, C>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        letroot!(rooted_self = self.clone(); [cx]);

        let flags = flags.unwrap_or(PropertyIteratorFlags::OWN_ONLY);
        let mut raw_keys = unsafe { IdVector::new(cx.as_raw_ptr()) };
        unsafe {
            GetPropertyKeys(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                flags.bits(),
                raw_keys.handle_mut(),
            )
        };
        ObjectKeysIterator::new(raw_keys)
    }
}

/// 'a is the lifetime of the root of the IdVector
pub struct ObjectKeysIterator<'a, C: Compartment> {
    raw_keys_slice: &'static [jsid],
    // `raw_keys` is never read since we use `raw_keys_slice` directly.
    // We however still rely on `IdVector` to keep the raw keys rooted
    // while iterating over them.
    #[allow(dead_code)]
    raw_keys: IdVector,
    index: usize,
    len: usize,
    marker: PhantomData<(C, &'a ())>,
}

impl<'a, C: Compartment> ObjectKeysIterator<'a, C> {
    fn new(raw_keys: IdVector) -> Self {
        let keys_slice = &*raw_keys;
        let len = keys_slice.len();
        let raw_keys_slice = unsafe { slice::from_raw_parts(keys_slice.as_ptr(), len) };
        Self {
            raw_keys_slice,
            raw_keys,
            index: 0,
            len,
            marker: PhantomData,
        }
    }
}

impl<'a, C: Compartment> Iterator for ObjectKeysIterator<'a, C> {
    type Item = PropertyKey<'a, C>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let raw_key = self.raw_keys_slice[self.index];
            self.index += 1;
            Some(PropertyKey::from_raw(raw_key))
        } else {
            None
        }
    }
}
