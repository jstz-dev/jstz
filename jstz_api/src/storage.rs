use std::ops::{DerefMut, Deref};

use boa_engine::{JsResult, Context, JsValue};
use boa_gc::{Trace, Finalize, empty_trace};
use jstz_core::host::Host;
use jstz_core::host_defined;
use tezos_smart_rollup_host::runtime::{RuntimeError, Runtime};
use tezos_smart_rollup_host::path::OwnedPath;
use jstz_core::kv::Transaction;
use jstz_core::host;
use serde::{Serialize, Deserialize};

use crate::conversion::{JsTypeError, ToJsError};

struct StoredPrefix(String);
unsafe impl Trace for StoredPrefix{
    empty_trace!();
}
impl Finalize for StoredPrefix{}

pub struct JsStorage<Host: Runtime, H : DerefMut<Target = Host>, T : DerefMut<Target = Transaction>, P : Deref<Target = StoredPrefix>> {
    host: H,
    transaction: T,
    prefix: P
}

#[derive(Debug, Serialize, Deserialize) ]
pub struct JsStoreValue(Vec<u8>);
impl jstz_core::kv::Value for JsStoreValue{}

impl<'a, Host: Runtime, H : DerefMut<Target = Host>, T : DerefMut<Target = Transaction>, P : Deref<Target = StoredPrefix>>
    JsStorage<Host,H,T,P>
{

    fn create_path(&self, name: &str) -> Result<OwnedPath, RuntimeError> {
        let prefix = &self.prefix.0;
        let path = format!("/{prefix}/{name}").to_string();
        path.try_into().map_err(|_| RuntimeError::PathNotFound)
    }

    fn write_value(
        &mut self,
        name: &str,
        payload: JsStoreValue
    ) -> Result<(), jstz_core::Error> {
        let path = self.create_path(name)?;
        self.transaction.insert(path, payload)
    }
    fn read_value(&self, name: &str) -> Result<Option<&JsStoreValue>, jstz_core::Error> {
        let path = self.create_path(name)?;
        self.transaction.get::<JsStoreValue>(*&self.host.deref(), path)
    }
    fn remove_value(&mut self, name: &str) -> Result<(), jstz_core::Error> {
        let path = self.create_path(name)?;
        self.transaction.remove(*&mut self.host.deref_mut(), &path)
    }
    fn has_value(&self, name: &str) -> Result<bool, jstz_core::Error> {
        let path = self.create_path(name)?;
        self.transaction.contains_key(self.host.deref(), &path)
    }
}


fn make_js_store<H : Runtime + 'static>(this: &JsValue, context: &mut Context) -> Option<JsStorage<Host<H>,impl DerefMut<Target = Host<H>>
                                                                                                                           , impl DerefMut<Target = Transaction>
                                                                                                          , impl Deref <Target = StoredPrefix>
                                                                                                                           >>{

    host_defined!(context, host_defined);
    let host = host_defined.get_mut::<Host<H>>()?;
    let transaction = host_defined.get_mut::<Transaction>()?.deref_mut();
    let prefix = this.as_object()
            .and_then(|obj| obj.downcast_mut::<StoredPrefix>())?;
    Some(JsStorage{host, transaction, prefix})

}
pub struct Contract{
    pub address: String,

}
