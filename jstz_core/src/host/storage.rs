use std::marker::PhantomData;

use crate::host::HostRef;
use jstz_serde::{Address, ByteRep, Byteable};
use tezos_smart_rollup_host::runtime::{RuntimeError, ValueType};
use tezos_smart_rollup_host::{path::OwnedPath, runtime::Runtime};

#[derive(Clone)]
pub struct Storage<Host> {
    host: HostRef<Host>,
    prefix: String,
}
impl<Host: Runtime + 'static> Storage<Host> {
    pub fn new(host: HostRef<Host>, prefix: String) -> Self {
        Self { host, prefix }
    }

    fn create_path(&self, name: &str) -> Result<OwnedPath, RuntimeError> {
        let prefix = &self.prefix;
        let path = format!("/{prefix}/{name}").to_string();
        path.try_into().map_err(|_| RuntimeError::PathNotFound)
    }

    pub fn write_value(
        &mut self,
        name: &str,
        payload: &[u8],
    ) -> Result<(), RuntimeError> {
        let path = self.create_path(name)?;
        self.host.store_write_all(&path, payload)
    }
    pub fn read_value(&self, name: &str) -> Result<Vec<u8>, RuntimeError> {
        let path = self.create_path(name)?;
        self.host.store_read_all(&path)
    }
    pub fn remove_value(&mut self, name: &str) -> Result<(), RuntimeError> {
        let path = self.create_path(name)?;
        self.host.store_delete(&path)
    }
    pub fn has_value(&self, name: &str) -> Result<Option<ValueType>, RuntimeError> {
        let path = self.create_path(name)?;
        self.host.store_has(&path)
    }
    pub fn prefix(&self) -> &String {
        &self.prefix
    }
    pub fn host(&self) -> &HostRef<Host> {
        &self.host
    }
}

#[derive(Clone)]
pub struct StorageMap<Host, Value> {
    data: Storage<Host>,
    phantom: PhantomData<Value>,
}
impl<Host: Runtime + 'static, Value: Byteable> StorageMap<Host, Value> {
    pub fn new(host: HostRef<Host>, prefix: String) -> Self {
        Self {
            data: Storage::new(host, prefix),
            phantom: PhantomData::default(),
        }
    }

    pub fn insert(
        &mut self,
        address: &Address,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let path = &address.to_string();
        let value = ByteRep::from_t(value);
        self.data.write_value(&path, value.bytes().as_slice())
    }
    pub fn get(&self, address: &Address) -> Result<Value, RuntimeError> {
        let path = &address.to_string();
        let value = ByteRep::new(self.data.read_value(path)?);
        value.into_t().map_err(|_| RuntimeError::DecodingError)
    }
    pub fn remove(&mut self, address: &Address) -> Result<(), RuntimeError> {
        let path = &address.to_string();
        self.data.remove_value(path)
    }
}
