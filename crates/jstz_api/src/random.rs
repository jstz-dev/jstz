use std::ops::BitXor;

use boa_engine::{
    js_string,
    object::{FunctionObjectBuilder, Object, ObjectInitializer},
    Context, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use bytes::Buf;
use jstz_crypto::{hash::Blake2b, public_key_hash::PublicKeyHash};

#[derive(Trace, Finalize)]
struct RandomGen {
    seed: u64,
}

impl RandomGen {
    fn from_js_value(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `RandomGen`")
                    .into()
            })
    }

    fn next(&mut self) -> JsValue {
        // fastrand's RNG does not implement trace,
        // so we extract and reinsert the seed when we call it
        let mut rng = fastrand::Rng::with_seed(self.seed);
        let result = rng.f64();
        self.seed = rng.get_seed();
        result.into()
    }
}

pub struct RandomApi {
    pub contract_address: PublicKeyHash,
    pub operation_hash: Blake2b,
}

impl RandomApi {
    fn to_random_gen(&self) -> RandomGen {
        let mut seed: u64 = 0;
        for byte in self
            .operation_hash
            .as_array()
            .chain(self.contract_address.as_bytes())
        {
            seed = seed.rotate_left(8).bitxor(byte as u64)
        }
        RandomGen { seed }
    }
    fn random(gen: &JsValue) -> JsResult<JsValue> {
        Ok(RandomGen::from_js_value(gen)?.next())
    }
}

impl jstz_core::Api for RandomApi {
    fn init(self, context: &mut Context) {
        let generator = ObjectInitializer::with_native(self.to_random_gen(), context)
            .build()
            .into();
        let random_method = FunctionObjectBuilder::new(
            context.realm(),
            NativeFunction::from_copy_closure_with_captures(
                |_, _, gen, _| Self::random(gen),
                generator,
            ),
        )
        .build();
        context
            .global_object()
            .get(js_string!("Math"), context)
            .expect("Math object not initialized")
            .as_object()
            .expect("Math should be an object")
            .set(js_string!("random"), random_method, false, context)
            .expect("Failed to set random number generator");
    }
}
