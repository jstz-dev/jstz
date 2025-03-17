use boa_engine::{
    js_string,
    object::{ErasedObject, FunctionObjectBuilder, ObjectInitializer},
    Context, JsData, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};

#[derive(Trace, Finalize, JsData)]
struct RandomGen {
    seed: u64,
}

impl RandomGen {
    fn from_js_value(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
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
    pub seed: u64,
}

impl RandomApi {
    fn random(gen: &JsValue) -> JsResult<JsValue> {
        Ok(RandomGen::from_js_value(gen)?.next())
    }
}

impl jstz_core::Api for RandomApi {
    fn init(self, context: &mut Context) {
        let generator =
            ObjectInitializer::with_native_data(RandomGen { seed: self.seed }, context)
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
