use std::{
    marker::PhantomData,
    pin::{pin, Pin},
    sync::Arc,
};

use mozjs::{
    jsapi::{JSObject, JS_HasPropertyById, JS_NewPlainObject},
    jsval::JSVal,
    rust::IntoHandle,
};

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    gc::ptr::GcPtr,
    letroot,
    value::JsValue,
    AsRawPtr,
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
                    self.inner_ptr.handle().into_handle(),
                    pkey.as_raw_handle().into_handle(),
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
        None
    }
}
