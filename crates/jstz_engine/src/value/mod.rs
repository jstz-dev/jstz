use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::jsval::JSVal;

use crate::{compartment::Compartment, gc::ptr::GcPtr};

pub struct JsValue<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<JSVal>>>,
    marker: PhantomData<(&'a (), C)>,
}

// pub enum JsVariant<'a, C> {
//     Null,
//     Undefined,
//     Integer32(u32),
//     Float64(f64),
//     Boolean(bool),
//     // Object(JsObject<'a, C>),
//     // String(JsString<'a, C>),
//     // Symbol(JsSymbol<'a, C>),
//     // BigInt(JsBigInt<'a, C>),
// }
