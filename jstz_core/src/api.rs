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

#[macro_export]
macro_rules! tezos_object {
    (mut $tezos:ident, $context:expr) => {
        tezos_object![INNER data, $context];
        let mut $tezos = jstz_core::api::TezosObject{data};
    };
    ($tezos:ident, $context:expr) => {
        tezos_object![INNER data, $context];
        let $tezos = jstz_core::api::TezosObject{data};
    };
    [INNER $data:ident, $context:expr] => {
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
            .get_mut::<jstz_core::api::TezosData>()
            .expect("TezosObject object not initialized");
    }
}

#[derive(Finalize)]
pub struct TezosData {
    pub self_address: Address,
    pub calling_address: Address,
    pub origin_address: Address,
    pub transaction: Transaction,
    pub kv_store: Kv,
}

impl TezosData {
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

unsafe impl Trace for TezosData {
    empty_trace!();
}

pub struct TezosObject<'a> {
    pub data: GcRefMut<'a, Box<dyn NativeObject + 'static>, TezosData>,
}

impl<'a> TezosObject<'a> {
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
    pub fn contract_call_data(&self, address: &Address) -> TezosData {
        self.data.contract_call_data(address)
    }
}
