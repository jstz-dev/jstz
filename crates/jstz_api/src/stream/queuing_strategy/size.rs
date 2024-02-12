use crate::{idl, stream::queuing_strategy::*, stream::Chunk};
use boa_engine::{object::NativeObject, Context, JsResult};
use boa_gc::{Finalize, Trace};
use jstz_core::{js_fn::JsCallableWithoutThis, native::JsNativeObject};

pub trait ExtractSizeAlgorithm {
    // The type of the extracted size algorithm. It could just be `QueuingStrategySizeAlgorithm`.
    // We allow it to vary to avoid wrapping in a constructor of the enum `QueuingStrategySizeAlgorithm` and immediately unwrapping when immediately calling the size algorithm.
    type ESA: Into<QueuingStrategySizeAlgorithm>
        + JsCallableWithoutThis<(Chunk,), idl::UnrestrictedDouble>;
    fn extract_size_algorithm(&self) -> Self::ESA;
}

impl ExtractSizeAlgorithm for DefaultQueuingStrategy {
    type ESA = CountQueuingStrategySizeAlgorithm;

    fn extract_size_algorithm(&self) -> Self::ESA {
        CountQueuingStrategySizeAlgorithm::ReturnOne
    }
}

#[derive(Default, Finalize, Trace)]
pub enum CountQueuingStrategySizeAlgorithm {
    #[default]
    ReturnOne,
}

impl CountQueuingStrategySizeAlgorithm {
    pub const RETURN_VALUE: idl::UnrestrictedDouble = 1.0;
}

impl JsCallableWithoutThis<(Chunk,), idl::UnrestrictedDouble>
    for CountQueuingStrategySizeAlgorithm
{
    fn call_without_this(
        &self,
        _inputs: (Chunk,),
        _context: &mut Context<'_>,
    ) -> JsResult<idl::UnrestrictedDouble> {
        match self {
            CountQueuingStrategySizeAlgorithm::ReturnOne => {
                Ok(CountQueuingStrategySizeAlgorithm::RETURN_VALUE)
            }
        }
    }
}

impl ExtractSizeAlgorithm for CountQueuingStrategy {
    type ESA = CountQueuingStrategySizeAlgorithm;

    fn extract_size_algorithm(&self) -> Self::ESA {
        CountQueuingStrategySizeAlgorithm::ReturnOne
    }
}

#[derive(Default, Finalize, Trace)]
pub enum ByteLengthQueuingStrategySizeAlgorithm {
    #[default]
    ReturnByteLengthOfChunk,
}

impl JsCallableWithoutThis<(Chunk,), idl::UnrestrictedDouble>
    for ByteLengthQueuingStrategySizeAlgorithm
{
    fn call_without_this(
        &self,
        _inputs: (Chunk,),
        _context: &mut Context<'_>,
    ) -> JsResult<idl::UnrestrictedDouble> {
        match self {
            ByteLengthQueuingStrategySizeAlgorithm::ReturnByteLengthOfChunk => {
                todo!("ReturnByteLengthOfChunk.call_without_this()")
            }
        }
    }
}

impl ExtractSizeAlgorithm for ByteLengthQueuingStrategy {
    type ESA = ByteLengthQueuingStrategySizeAlgorithm;

    fn extract_size_algorithm(&self) -> Self::ESA {
        ByteLengthQueuingStrategySizeAlgorithm::ReturnByteLengthOfChunk
    }
}

impl<T: NativeObject + ExtractSizeAlgorithm> ExtractSizeAlgorithm for JsNativeObject<T> {
    type ESA = T::ESA;

    fn extract_size_algorithm(&self) -> Self::ESA {
        self.deref().extract_size_algorithm()
    }
}

#[derive(From)]
pub enum QueuingStrategySizeAlgorithm {
    Count(CountQueuingStrategySizeAlgorithm),
    ByteLength(ByteLengthQueuingStrategySizeAlgorithm),
    Custom(crate::todo::Todo),
}

impl JsCallableWithoutThis<(Chunk,), idl::UnrestrictedDouble>
    for QueuingStrategySizeAlgorithm
{
    fn call_without_this(
        &self,
        inputs: (Chunk,),
        context: &mut Context<'_>,
    ) -> JsResult<idl::UnrestrictedDouble> {
        match self {
            QueuingStrategySizeAlgorithm::Count(size_algorithm) => {
                size_algorithm.call_without_this(inputs, context)
            }
            QueuingStrategySizeAlgorithm::ByteLength(size_algorithm) => {
                size_algorithm.call_without_this(inputs, context)
            }
            QueuingStrategySizeAlgorithm::Custom(_size_algorithm) => {
                todo!("custom_size_algorithm.call_without_this(inputs, context)")
            }
        }
    }
}

impl ExtractSizeAlgorithm for QueuingStrategy {
    type ESA = QueuingStrategySizeAlgorithm;

    fn extract_size_algorithm(&self) -> Self::ESA {
        match self {
            QueuingStrategy::Default(strategy) => {
                strategy.extract_size_algorithm().into()
            }
            QueuingStrategy::Count(strategy) => strategy.extract_size_algorithm().into(),
            QueuingStrategy::ByteLength(strategy) => {
                strategy.extract_size_algorithm().into()
            }
            QueuingStrategy::Custom(_strategy) => {
                todo!("custom_queuing_strategy.extract_size_algorithm()")
            }
        }
    }
}

impl Default for QueuingStrategySizeAlgorithm {
    fn default() -> Self {
        CountQueuingStrategySizeAlgorithm::ReturnOne.into()
    }
}

impl ExtractSizeAlgorithm for Option<QueuingStrategy> {
    type ESA = QueuingStrategySizeAlgorithm;

    fn extract_size_algorithm(&self) -> Self::ESA {
        self.as_ref()
            .map(|v| v.extract_size_algorithm())
            .unwrap_or_default()
    }
}
