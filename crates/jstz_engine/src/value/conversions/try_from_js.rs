use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::Compartment,
    value::JsValue,
};

/// This trait adds a fallible and efficient conversions from a [`JsValue`] to Rust types.
/// 
/// Note that the Output type is not directly linked to 
pub trait TryFromJs<'a, 'cx: 'a, C: Compartment> {
    type Output;
    type Config;

    /// This function tries to convert a JavaScript value into `Self`.
    /// `config` determines the behaviour of the conversion.
    fn try_from_js<S>(
        value: &JsValue<'a, C>,
        config: Self::Config,
        cx: &'cx mut Context<S>,
    ) -> Option<Self::Output>
    where
        S: InCompartment<C> + CanAccess + CanAlloc;
}
