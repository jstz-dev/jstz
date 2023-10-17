use boa_engine::Context;
use jstz_core::{
    api::{GlobalApi, JstzData},
    Realm,
};

pub mod contract;
mod ledger;

pub use ledger::LedgerApi;

pub struct Api;

impl GlobalApi for Api {
    fn init(context: &mut Context) {
        jstz_api::Api::init(context);
        contract::Api::init(context);
        // ledger::Api::init(context);
    }
}
pub fn initialize_apis(
    contract_parameters: JstzData,
    realm: &Realm,
    context: &mut Context,
) {
    // 1. Get the correct context for the realm
    let context = &mut realm.context_handle(context);

    // 2. Add the jstz object to the context
    let None = contract_parameters.insert_into_context(context)
        else {panic!("JstzApi already initialized")};

    // 3. initialize
    Api::init(context);
}
