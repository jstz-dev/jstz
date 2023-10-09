use crate::{
    host_defined,
    kv::{Kv, Transaction},
};
use boa_engine::{object::NativeObject, Context};
use boa_gc::GcRefMut;
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_crypto::public_key_hash::PublicKeyHash as Address;

/// A generic runtime API
/// TODO deprecate in favour of GlobalApi
/// (no name clash + remove self parameter)
pub trait Api {
    /// Initialize a runtime API
    fn init(self, context: &mut Context);
}

pub trait GlobalApi {
    fn init(context: &mut Context);
}

/// macro to allow access to the jstz object and host runtime
/// Usage:
/// Takes a context and a single function call as if there
/// were variables jstz : Jstz and hrt : Runtime in scope.
/// with_jstz!(context, my_function(&jstz, &mut hrt, my_variable ))
/// limitations:
///  * jstz and hrt must be passed by reference
///  * jstz must be the first parameter
///  * hrt must be either the first or, if jstz is present, the second parameter
///  * if the function is an expression not an identifier it must be in parens
///     -  with_jstz!(context, (my_class.method)(&mut jstz))
///  * if the function is more complex it must be in square brackets
///     -  with_jstz!(context, [MyClass::static_method](&mut jstz))
///  * requires a trailing comma if only jstz and hrt arguments are used
#[macro_export]
macro_rules! with_jstz {
    ($context:expr,$func:ident($($inner:tt)*)) => {
        with_jstz![INTERNAL $context;[$func];$($inner)*]
    };
    ($context:expr,($func:expr)($($inner:tt)*)) => {
        with_jstz![INTERNAL $context;[$func];$($inner)*]
    };
    ($context:expr,[$($func:tt)*]($($inner:tt)*)) => {
        with_jstz![INTERNAL $context;[$($func)*];$($inner)*]
    };

    [INTERNAL $context:expr;[$($func:tt)*]; &mut jstz, $($tail:tt)* ] => {
        {
        with_jstz![HOST_DEFINED $context;data];
        let mut jstz = jstz_core::api::Jstz{data};
        with_jstz![CALL_WITH_RUNTIME [&mut jstz]; ($($func)*); $($tail)*]
        }
    };
    [INTERNAL $context:expr;[$($func:tt)*]; &jstz, $($tail:tt)* ] => {
        {
        with_jstz![HOST_DEFINED $context;data];
        let jstz = jstz_core::api::Jstz{data};
        with_jstz![CALL_WITH_RUNTIME [&jstz]; ($($func)*); $($tail)* ]
        }

    };
    [INTERNAL $context:expr;[$($func:tt)*]; $($tail:tt)* ] => {
        {
        with_jstz!(HOST_DEFINED $context;data);
        let jstz = jstz_core::api::Jstz{data};
        with_jstz![CALL_WITH_RUNTIME ; ($($func)*) ; $($tail)*]
        }

    };
    [HOST_DEFINED $context:expr;$data:ident] => {
        let host_defined_binding = $context
            .global_object()
            .get(js_string!(jstz_core::realm::HostDefined::NAME), $context)
            .expect(&format!(
                "{:?} should be defined",
                jstz_core::realm::HostDefined::NAME
            ));
        let host_defined = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<jstz_core::realm::HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");
        let $data = host_defined
            .get_mut::<jstz_core::api::JstzData>()
            .expect("Jstz object not initialized");
    };
    [CALL_WITH_RUNTIME $([$jstz:expr])? ; ($($func:tt)*); & hrt, $($tail:tt)*] => {
        jstz_core::runtime::with_global_host(|hrt| $($func)*($($jstz,)? &*hrt, $($tail)*))
    };
    [CALL_WITH_RUNTIME $([$jstz:expr])? ; ($($func:tt)*); &mut hrt, $($tail:tt)*] => {
        jstz_core::runtime::with_global_host(|hrt| $($func)*($($jstz,)? hrt, $($tail)*))
    };
    [CALL_WITH_RUNTIME $([$jstz:expr])? ; ($($func:tt)*); $($tail:tt)*] => {
        $($func)*($($jstz,)? $($tail)*)
    }
}

#[derive(Finalize)]
pub struct JstzData {
    pub self_address: Address,
    pub calling_address: Address,
    pub origin_address: Address,
    pub transaction: Transaction,
    pub kv_store: Kv,
}

impl JstzData {
    pub fn contract_call_data(&self, address: &Address) -> Self {
        let kv_store = Kv::new();
        let transaction = kv_store.begin_transaction();
        Self {
            self_address: address.clone(),
            calling_address: self.self_address.clone(),
            origin_address: self.origin_address.clone(),
            kv_store,
            transaction,
        }
    }
    pub fn insert_into_context(self, context: &mut Context) -> Option<Self> {
        host_defined!(context, mut host_defined);
        Some(*host_defined.insert(self)?)
    }
    pub fn remove_from_context(context: &mut Context) -> Option<Self> {
        host_defined!(context, mut host_defined);
        Some(*host_defined.remove()?)
    }
}

unsafe impl Trace for JstzData {
    empty_trace!();
}

pub struct Jstz<'a> {
    pub data: GcRefMut<'a, Box<dyn NativeObject + 'static>, JstzData>,
}

impl<'a> Jstz<'a> {
    pub fn self_address<'b>(&'b self) -> &'b Address {
        &self.data.self_address
    }
    pub fn calling_address<'b>(&'b self) -> &'b Address {
        &self.data.calling_address
    }
    pub fn origin_address<'b>(&'b self) -> &'b Address {
        &self.data.origin_address
    }
    pub fn transaction<'b>(&'b self) -> &'b Transaction {
        &self.data.transaction
    }
    pub fn transaction_mut<'b>(&'b mut self) -> &'b mut Transaction {
        &mut self.data.transaction
    }
    pub fn kv_store<'b>(&'b self) -> &'b Kv {
        &self.data.kv_store
    }
    pub fn kv_store_mut<'b>(&'b mut self) -> &'b mut Kv {
        &mut self.data.kv_store
    }
    pub fn contract_call_data(&self, address: &Address) -> JstzData {
        self.data.contract_call_data(address)
    }
}
