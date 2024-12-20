use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::{
    jsapi::{
        JSObject, JS_GetPropertyById, JS_HasPropertyById, JS_NewPlainObject,
        JS_SetPropertyById,
    },
    jsval::UndefinedValue,
    rooted,
    rust::IntoHandle,
};

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    ffi::AsRawPtr,
    gc::ptr::{GcPtr, Handle, HandleMut},
    letroot,
    value::JsValue,
};

use super::property::IntoPropertyKey;

/// Represents an [`Object`] in the JavaScript engine.
///
/// Refer to [MDN](..)
pub struct JsObject<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<*mut JSObject>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> JsObject<'a, C> {
    pub(crate) unsafe fn handle(&self) -> Handle<*mut JSObject> {
        self.inner_ptr.handle()
    }

    pub(crate) unsafe fn handle_mut(&self) -> HandleMut<*mut JSObject> {
        self.inner_ptr.handle_mut()
    }

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

    pub fn has<'cx, S, K>(&self, key: K, cx: &'cx mut Context<S>) -> bool
    where
        'a: 'cx,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey<'cx, C>,
    {
        letroot!(pkey = key.into_key(cx); [cx]);

        if let Some(pkey) = pkey.as_ref() {
            let mut found = false;

            if unsafe {
                JS_HasPropertyById(
                    cx.as_raw_ptr(),
                    self.handle(),
                    pkey.handle(),
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

    pub fn get<'cx, S, K>(
        &self,
        key: K,
        cx: &'cx mut Context<S>,
    ) -> Option<JsValue<'cx, C>>
    where
        'a: 'cx,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey<'cx, C>,
    {
        letroot!(pkey = key.into_key(cx)?; [cx]);
        letroot!(rval = JsValue::undefined(cx); [cx]);

        let res = unsafe {
            JS_GetPropertyById(
                cx.as_raw_ptr(),
                self.handle(),
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

    pub fn set<'b, 'cx, S, K>(
        &self,
        key: K,
        value: &JsValue<'b, C>,
        cx: &'cx mut Context<S>,
    ) where
        'a: 'cx,
        'b: 'cx,
        S: InCompartment<C> + CanAlloc,
        K: IntoPropertyKey<'cx, C>,
    {
        letroot!(pkey = key.into_key(cx).unwrap(); [cx]);

        unsafe {
            JS_SetPropertyById(
                cx.as_raw_ptr(),
                self.handle().into(),
                pkey.handle().into(),
                value.handle().into(),
            )
        };
    }
}
