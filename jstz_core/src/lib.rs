use std::marker::PhantomData;

use boa_engine::{context::ContextBuilder, Context, Source};
use host::{Host, HostDefined};
use kv::{Kv, Transaction};
use tezos_smart_rollup_host::runtime::Runtime;

mod error;

pub use error::{Error, Result};
pub mod host;
pub mod kv;

pub struct JstzRuntime<'host, H: Runtime + 'static> {
    context: Context<'host>,
    _host: PhantomData<H>,
}

impl<'a, H> JstzRuntime<'a, H>
where
    H: Runtime + 'static,
{
    pub fn new(rt: &mut H) -> Self {
        let mut context = ContextBuilder::new()
            .host_hooks(host::HOOKS)
            .build()
            .unwrap();

        // Setup host-defined objects
        let kv = Kv::new();
        let tx = kv.begin_transaction();
        let host = unsafe { Host::new(rt) };

        // Setup `HostDefined` table
        let mut host_defined = HostDefined::new();
        host_defined.insert(host);
        host_defined.insert(kv);
        host_defined.insert(tx);

        host_defined.init(&mut context);

        Self {
            context,
            _host: PhantomData,
        }
    }

    pub fn register_global_api<T>(&mut self)
    where
        T: host::Api,
    {
        T::init::<H>(&mut self.context)
    }

    pub fn eval(mut self, src: impl AsRef<[u8]>) -> String {
        let result = self
            .context
            .eval(Source::from_bytes(&src))
            .map(|v| v.display().to_string())
            .unwrap_or("Uncaught error".to_string());

        host_defined!(&mut self.context, mut host_defined);

        let mut host = host_defined
            .remove::<Host<H>>()
            .expect("Rust type `Host<H>` should be defined in `HostDefined`");

        let mut kv = host_defined
            .remove::<Kv>()
            .expect("Rust type `Kv` should be defined in `HostDefined`");

        let tx = host_defined
            .remove::<Transaction>()
            .expect("Rust type `Transaction` should be defined in `HostDefined`");

        kv.commit_transaction(&mut *host, *tx)
            .expect("Failed to commit transaction!");

        result
    }
}

// JS eval function
pub fn evaluate_from_bytes<H: Runtime + 'static>(
    rt: &mut H,
    src: impl AsRef<[u8]>,
) -> String {
    JstzRuntime::new(rt).eval(src)
}
