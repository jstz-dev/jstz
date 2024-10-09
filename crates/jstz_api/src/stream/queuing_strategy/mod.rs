//! [Streams Standard - § 7. Queuing strategies][https://streams.spec.whatwg.org/#qs]
//!
//! A few things to keep in mind:
//!
//! - The queuing strategy given to a stream constructor can be either builtin one (i.e. an instance of `CountQueuingStrategy` or `ByteLengthQueuingStrategy`)
//!   or a custom one (i.e. anything else).
//!
//! - While builtin queuing strategies always have a `highWaterMark` property, custom queuing strategies may not.
//!   When the `highWaterMark` property is missing, a default value is used instead. This default value depends on
//!   the kind of stream being built, as can be seen by looking at the various call to `ExtractHighWaterMark(strategy, defaultHWM)` in the specification.
//!
//! - The default queuing strategy is supposed to behave as if it were `new CountQueuingStrategy({highWaterMark: 1.0})`.
//!

use boa_engine::{value::TryFromJs, Context, JsResult, JsValue};
use derive_more::*;
use jstz_core::native::{register_global_class, JsNativeObject};

use crate::stream::queuing_strategy::builtin::{
    ByteLengthQueuingStrategy, ByteLengthQueuingStrategyClass, CountQueuingStrategy,
    CountQueuingStrategyClass,
};

pub mod builtin;
pub mod high_water_mark;
pub mod size;

#[derive(Default)]
pub struct DefaultQueuingStrategy {}

#[derive(From)]
pub enum QueuingStrategy {
    Default(DefaultQueuingStrategy),
    Count(JsNativeObject<CountQueuingStrategy>),
    ByteLength(JsNativeObject<ByteLengthQueuingStrategy>),
    Custom(crate::todo::Todo),
}

impl Default for QueuingStrategy {
    fn default() -> Self {
        (DefaultQueuingStrategy {}).into()
    }
}

impl TryFromJs for QueuingStrategy {
    fn try_from_js(value: &JsValue, _context: &mut Context<'_>) -> JsResult<Self> {
        if JsNativeObject::<CountQueuingStrategy>::is(value) {
            JsNativeObject::<CountQueuingStrategy>::try_from(value.clone())
                .map(Into::into)
        } else if JsNativeObject::<ByteLengthQueuingStrategy>::is(value) {
            JsNativeObject::<ByteLengthQueuingStrategy>::try_from(value.clone())
                .map(Into::into)
        } else {
            todo!("try_from_js(custom_queuing_strategy)")
        }
    }
}

pub struct QueuingStrategyApi;

impl jstz_core::Api for QueuingStrategyApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<CountQueuingStrategyClass>(context)
            .expect("The `CountQueuingStrategy` class shouldn't exist yet");
        register_global_class::<ByteLengthQueuingStrategyClass>(context)
            .expect("The `ByteLengthQueuingStrategy` class shouldn't exist yet");
    }
}
