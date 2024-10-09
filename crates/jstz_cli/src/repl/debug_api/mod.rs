use boa_engine::{js_string, object::ObjectInitializer, property::Attribute, Context};

mod account;
mod kv;

pub struct DebugApi;

impl DebugApi {
    const NAME: &'static str = "jstz";
}

impl jstz_core::Api for DebugApi {
    fn init(self, context: &mut Context) {
        let kv_api = kv::KvApi::namespace(context);
        let account_api = account::AccountApi::namespace(context);

        let storage = ObjectInitializer::new(context)
            .property(js_string!("Kv"), kv_api, Attribute::all())
            .property(js_string!("Account"), account_api, Attribute::all())
            .build();

        context
            .register_global_property(js_string!(Self::NAME), storage, Attribute::all())
            .expect("The storage object shouldn't exist yet");
    }
}
