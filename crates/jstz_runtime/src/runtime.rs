use std::ops::Deref;
use std::ops::DerefMut;

use crate::init_ops_and_esm_extensions;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use jstz_core::host::{HostRuntime, JsHostRuntime};

fn init_extenions() -> Vec<Extension> {
    init_ops_and_esm_extensions!()
}

pub type JstzHostRuntime = JsHostRuntime<'static>;

/// [`JstzRuntime`] manages the [`JsRuntime`] state. It is also
/// provides [`JsRuntime`] with the instiatiated [`HostRuntime`]
/// and protocol capabilities
pub struct JstzRuntime {
    pub runtime: JsRuntime,
}

impl JstzRuntime {
    pub fn init(host_runtime: &mut impl HostRuntime) -> Self {
        // Register host runtime
        let hrt = JsHostRuntime::new(host_runtime);
        let register_hrt_ext = Extension {
            name: "register_hrt_ext",
            op_state_fn: Some(Box::new(|op_state| {
                op_state.put::<JstzHostRuntime>(hrt);
            })),
            ..Default::default()
        };

        // Initialize extensions
        let mut extensions = vec![register_hrt_ext];
        extensions.extend(init_extenions());

        let runtime = JsRuntime::new(RuntimeOptions {
            extensions,
            ..Default::default()
        });

        Self { runtime }
    }
}

impl Deref for JstzRuntime {
    type Target = JsRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl DerefMut for JstzRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

#[macro_export]
macro_rules! init_ops_and_esm_extensions  {
    ($($ext:ident),*) => {
        vec![
            $($ext::init_ops_and_esm()),*
        ]
    };
}
