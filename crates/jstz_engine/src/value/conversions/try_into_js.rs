use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::Compartment,
    value::JsValue,
};

/// This trait adds a conversion from a Rust Type into a [`JsValue`]
pub trait TryIntoJs<'a, 'cx: 'a, C: Compartment> {
    type Output;

    /// This function tries to convert a `Self` into a [`JsValue`].
    fn try_into_js<S>(&self, context: &'cx mut Context<S>) -> Option<JsValue<'cx, C>>
    where
        S: InCompartment<C> + CanAccess + CanAlloc;
}
