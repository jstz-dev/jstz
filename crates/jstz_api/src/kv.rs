use std::ops::{Deref, DerefMut};

use boa_engine::{
    js_string, object::ObjectInitializer, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, realm::HostDefined, runtime, Result,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};

#[derive(Debug, Trace, Finalize)]
pub struct Kv {
    prefix: String,
}

const KV_PATH: RefPath = RefPath::assert_from(b"/jstz_kv");

// TODO: Figure out a more effective way of serializing values using json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KvValue(pub serde_json::Value);

impl From<KvValue> for String {
    fn from(val: KvValue) -> Self {
        val.0.to_string()
    }
}

impl TryFrom<String> for KvValue {
    type Error = serde_json::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self(serde_json::from_str(&value)?))
    }
}

impl Kv {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }

    fn key_path(&self, key: &str) -> jstz_core::Result<OwnedPath> {
        let key_path = OwnedPath::try_from(format!("/{}/{}", self.prefix, key))?;

        Ok(path::concat(&KV_PATH, &key_path)?)
    }

    pub fn set(&self, tx: &mut Transaction, key: &str, value: KvValue) -> Result<()> {
        tx.insert(self.key_path(key)?, value)
    }

    pub fn get<'a>(
        &self,
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        key: &str,
    ) -> Result<Option<&'a KvValue>> {
        tx.get::<KvValue>(hrt, self.key_path(key)?)
    }

    pub fn delete(
        &self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        key: &str,
    ) -> Result<()> {
        tx.remove(hrt, &self.key_path(key)?)
    }

    pub fn has(
        &self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        key: &str,
    ) -> Result<bool> {
        tx.contains_key(hrt, &self.key_path(key)?)
    }
}

macro_rules! preamble {
    ($this:ident, $args:ident, $context:ident, $key:ident, $tx:ident) => {
        host_defined!($context, host_defined);
        let mut $tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");

        let $this = $this
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Kv>())
            .ok_or_else(|| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to convert js value into rust type `Kv`"),
                )
            })?;

        let $key = $args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `String`")
            })
            .map(JsString::to_std_string_escaped)?;
    };
}

macro_rules! preamble_static {
    ($this:ident, $args:ident, $context:ident, $key:ident, $tx:ident) => {
        let host_defined_binding = $context
            .global_object()
            .get(js_string!(HostDefined::NAME), $context)
            .expect(&format!("{:?} should be defined", HostDefined::NAME));

        let mut host_defined: GcRefMut<'_, _, HostDefined> = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");

        let mut $tx: GcRefMut<'_, _, Transaction> =
            HostDefined::get_mut::<Transaction>(host_defined.deref_mut())
                .expect("Curent transaction undefined");

        let $this = $this
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Kv>())
            .ok_or_else(|| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to convert js value into rust type `Kv`"),
                )
            })?;

        let $key = $args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `String`")
            })
            .map(JsString::to_std_string_escaped)?;
    };
}

pub struct KvApi {
    pub contract_address: PublicKeyHash,
}

impl KvApi {
    const NAME: &'static str = "Kv";

    fn set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let value = KvValue(args.get_or_undefined(1).to_json(context)?);

        this.set(&mut tx, &key, value)?;

        Ok(JsValue::undefined())
    }

    fn get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble_static!(this, args, context, key, tx);

        let result =
            runtime::with_global_host(|rt| this.get(rt.deref(), tx.deref_mut(), &key))?;

        match result {
            Some(value) => JsValue::from_json(&value.0, context),
            None => Ok(JsValue::null()),
        }
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble_static!(this, args, context, key, tx);

        runtime::with_global_host(|hrt| this.delete(hrt.deref(), tx.deref_mut(), &key))?;

        Ok(JsValue::undefined())
    }

    fn has(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let result =
            runtime::with_global_host(|hrt| this.has(hrt.deref(), &mut tx, &key))?;

        Ok(result.into())
    }
}

impl jstz_core::Api for KvApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let storage = ObjectInitializer::with_native(
            Kv::new(self.contract_address.to_string()),
            context,
        )
        .function(NativeFunction::from_fn_ptr(Self::set), js_string!("set"), 2)
        .function(NativeFunction::from_fn_ptr(Self::get), js_string!("get"), 1)
        .function(
            NativeFunction::from_fn_ptr(Self::delete),
            js_string!("delete"),
            1,
        )
        .function(NativeFunction::from_fn_ptr(Self::has), js_string!("has"), 1)
        .build();

        context
            .register_global_property(js_string!(Self::NAME), storage, Attribute::all())
            .expect("The storage object shouldn't exist yet");
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use jstz_core::kv;
    use jstz_crypto::keypair_from_passphrase;
    use jstz_proto::context::account::Account;
    use tezos_smart_rollup_mock::MockHost;

    fn get_random_public_key_hash(passphrase: &str) -> PublicKeyHash {
        let (_, pk) =
            keypair_from_passphrase(passphrase).expect("Failed to generate keypair");
        return PublicKeyHash::try_from(&pk)
            .expect("Failed to generate public key hash.");
    }

    fn get_account_balance_from_storage(
        hrt: &impl HostRuntime,
        pkh: &PublicKeyHash,
    ) -> u64 {
        let account = match kv::Storage::get::<Account>(
            hrt,
            &Account::path(&pkh).expect("Could not get path"),
        )
        .expect("Could not find the account")
        {
            Some(account) => account,
            None => panic!("Account not found"),
        };

        account.amount
    }

    fn verify_account_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        pkh: &PublicKeyHash,
        expected: u64,
    ) {
        let amt = Account::balance(hrt, tx, &pkh).expect("Could not get balance");

        assert_eq!(amt, expected);
    }

    fn commit_transaction_mock(hrt: &mut MockHost, tx: &Rc<RefCell<Transaction>>) {
        tx.deref()
            .borrow_mut()
            .commit::<Account>(hrt)
            .expect("Could not commit tx");
    }

    #[test]
    fn test_nested_transactions() -> Result<()> {
        let hrt = &mut MockHost::default();
        let tx = Rc::new(RefCell::new(Transaction::new()));
        let pkh1 = get_random_public_key_hash("passphrase1");
        let pkh2 = get_random_public_key_hash("passphrase2");

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 0);
        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh2, 0);

        let child_tx = Transaction::begin(Rc::clone(&tx));

        let _ = Account::deposit(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 0);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);
        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh2, 0);

        let grandchild_tx = Transaction::begin(Rc::clone(&child_tx));

        verify_account_balance(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh2, 25);

        let _ = Account::deposit(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh1, 57);

        commit_transaction_mock(hrt, &grandchild_tx);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);

        let _ = Account::deposit(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 2 * 57);

        commit_transaction_mock(hrt, &child_tx);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 2 * 57);

        let _ = Account::deposit(hrt, &mut tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 3 * 57);

        commit_transaction_mock(hrt, &tx);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 3 * 57);

        assert_eq!(get_account_balance_from_storage(hrt, &pkh1), 3 * 57);

        assert_eq!(get_account_balance_from_storage(hrt, &pkh2), 25);

        Ok(())
    }
}
