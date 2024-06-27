use crate::idl;
use crate::stream::queuing_strategy::DefaultQueuingStrategy;
use crate::stream::queuing_strategy::{
    builtin::{ByteLengthQueuingStrategy, CountQueuingStrategy},
    QueuingStrategy,
};
use boa_engine::{object::NativeObject, JsError, JsNativeError, JsResult};
use jstz_core::native::JsNativeObject;

/// [Streams Standard - § 7.4.][https://streams.spec.whatwg.org/#validate-and-normalize-high-water-mark]
/// > `ExtractHighWaterMark(strategy, defaultHWM)`
pub trait ExtractHighWaterMark {
    fn extract_high_water_mark(
        &self,
        default_hwm: HighWaterMark,
    ) -> JsResult<HighWaterMark>;
}

impl ExtractHighWaterMark for DefaultQueuingStrategy {
    fn extract_high_water_mark(
        &self,
        _default_hwm: HighWaterMark,
    ) -> JsResult<HighWaterMark> {
        Ok(HighWaterMark::ONE)
    }
}

macro_rules! impl_extract_high_water_mark_for_builtin_queuing_strategy {
    ($T: ty) => {
        impl ExtractHighWaterMark for $T {
            fn extract_high_water_mark(
                &self,
                _default_hwm: HighWaterMark,
            ) -> JsResult<HighWaterMark> {
                HighWaterMark::try_from(self.high_water_mark)
            }
        }
    };
}

impl_extract_high_water_mark_for_builtin_queuing_strategy!(CountQueuingStrategy);
impl_extract_high_water_mark_for_builtin_queuing_strategy!(ByteLengthQueuingStrategy);

impl<T: NativeObject + ExtractHighWaterMark> ExtractHighWaterMark for JsNativeObject<T> {
    fn extract_high_water_mark(
        &self,
        default_hwm: HighWaterMark,
    ) -> JsResult<HighWaterMark> {
        self.deref().extract_high_water_mark(default_hwm)
    }
}

impl ExtractHighWaterMark for QueuingStrategy {
    fn extract_high_water_mark(
        &self,
        default_hwm: HighWaterMark,
    ) -> JsResult<HighWaterMark> {
        match self {
            QueuingStrategy::Default(strategy) => {
                strategy.extract_high_water_mark(default_hwm)
            }
            QueuingStrategy::Count(strategy) => {
                strategy.extract_high_water_mark(default_hwm)
            }
            QueuingStrategy::ByteLength(strategy) => {
                strategy.extract_high_water_mark(default_hwm)
            }
            QueuingStrategy::Custom(_strategy) => {
                todo!("custom_queuing_strategy.extract_high_water_mark(default_hwm)")
            }
        }
    }
}

/// A subtype of `idl::UnrestrictedDouble` that only containts values that are neither `NaN` nor negative.
pub struct HighWaterMark {
    inner: idl::UnrestrictedDouble,
}

impl TryFrom<idl::UnrestrictedDouble> for HighWaterMark {
    type Error = JsError;

    fn try_from(value: idl::UnrestrictedDouble) -> Result<Self, Self::Error> {
        if value.is_nan() || value < 0.0 {
            return Err(JsNativeError::range()
                .with_message("Invalid highWaterMark")
                .into());
        }
        Ok(HighWaterMark { inner: value })
    }
}

impl From<HighWaterMark> for idl::UnrestrictedDouble {
    fn from(value: HighWaterMark) -> idl::UnrestrictedDouble {
        value.inner
    }
}

impl HighWaterMark {
    pub const ZERO: HighWaterMark = HighWaterMark { inner: 0.0 };
    pub const ONE: HighWaterMark = HighWaterMark { inner: 1.0 };
    pub const INFINITY: HighWaterMark = HighWaterMark {
        inner: idl::UnrestrictedDouble::INFINITY,
    };
}

impl Default for HighWaterMark {
    fn default() -> Self {
        HighWaterMark::INFINITY
    }
}
