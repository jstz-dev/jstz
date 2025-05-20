mod kv;
mod ledger;

use std::ops::BitXor;

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};
use jstz_core::host_defined;
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use kv::KvApi;
use ledger::LedgerApi;

use crate::{operation::OperationHash, runtime::v1::api};

pub use kv::{Kv, KvValue};

#[derive(Trace, Finalize, JsData)]
pub struct ProtocolData {
    pub address: SmartFunctionHash,
    pub operation_hash: OperationHash,
}

pub struct WebApi;

impl jstz_core::Api for WebApi {
    fn init(self, context: &mut boa_engine::Context) {
        use jstz_api::*;

        url::UrlApi.init(context);
        urlpattern::UrlPatternApi.init(context);
        http::HttpApi.init(context);
        encoding::EncodingApi.init(context);
        ConsoleApi.init(context);
        file::FileApi.init(context);
    }
}

pub struct ProtocolApi {
    pub address: SmartFunctionHash,
    pub operation_hash: OperationHash,
}

impl jstz_core::Api for ProtocolApi {
    fn init(self, context: &mut boa_engine::Context) {
        WebApi.init(context);

        host_defined!(context, mut host_defined);
        host_defined.insert(ProtocolData {
            address: self.address.clone(),
            operation_hash: self.operation_hash.clone(),
        });

        jstz_api::RandomApi {
            seed: compute_seed(&self.address, &self.operation_hash),
        }
        .init(context);

        api::KvApi {
            address: self.address.clone(),
        }
        .init(context);

        api::LedgerApi {
            address: self.address.clone(),
        }
        .init(context);
        crate::api::smart_function::SmartFunctionApi {
            address: self.address.clone(),
        }
        .init(context);
    }
}

fn compute_seed(address: &SmartFunctionHash, operation_hash: &OperationHash) -> u64 {
    let mut seed: u64 = 0;
    for byte in operation_hash.as_array().iter().chain(address.as_bytes()) {
        seed = seed.rotate_left(8).bitxor(*byte as u64)
    }

    seed
}
