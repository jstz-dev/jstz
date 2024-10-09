use boa_engine::{value::TryFromJs, Context, JsArgs, JsData, JsResult, JsValue};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::native::{register_global_class, ClassBuilder, NativeClass};

use crate::stream::{
    queuing_strategy::{
        high_water_mark::{ExtractHighWaterMark, HighWaterMark},
        size::ExtractSizeAlgorithm,
        QueuingStrategy,
    },
    readable::underlying_source::{ReadableStreamType, UnderlyingSource},
};

pub mod underlying_source;

#[derive(JsData)]
pub struct ReadableStream {
    // TODO
}

impl Finalize for ReadableStream {
    fn finalize(&self) {
        todo!()
    }
}

unsafe impl Trace for ReadableStream {
    custom_trace!(this, _mark, {
        let _ = this;
        todo!()
    });
}

pub struct ReadableStreamClass;

impl NativeClass for ReadableStreamClass {
    type Instance = ReadableStream;

    const NAME: &'static str = "ReadableStream";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<Self::Instance> {
        let underlying_source =
            Option::<UnderlyingSource>::try_from_js(args.get_or_undefined(0), context)?
                .unwrap_or_else(|| todo!());
        let queuing_strategy =
            Option::<QueuingStrategy>::try_from_js(args.get_or_undefined(1), context)?
                .unwrap_or_default();

        // TODO Perform ! InitializeReadableStream(this).

        if underlying_source.r#type == Some(ReadableStreamType::Bytes) {
            // TODO If strategy["size"] exists, throw a RangeError exception.
            let high_water_mark =
                queuing_strategy.extract_high_water_mark(HighWaterMark::ZERO)?;
            let _ = high_water_mark;
            todo!("SetUpReadableByteStreamControllerFromUnderlyingSource")
        } else {
            // TODO Assert: underlyingSourceDict["type"] does not exist.
            let size_algorithm = queuing_strategy.extract_size_algorithm();
            let high_water_mark =
                queuing_strategy.extract_high_water_mark(HighWaterMark::ONE)?;
            let _ = (high_water_mark, size_algorithm);
            todo!("SetUpReadableStreamDefaultControllerFromUnderlyingSource")
        }
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        // TODO
        let _ = class;
        Ok(())
    }
}

pub struct ReadableStreamApi;

impl jstz_core::Api for ReadableStreamApi {
    fn init(self, context: &mut Context) {
        register_global_class::<ReadableStreamClass>(context)
            .expect("The `ReadableStream` class shouldn't exist yet")
        // TODO
    }
}
