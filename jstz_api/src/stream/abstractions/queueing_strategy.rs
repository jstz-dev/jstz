use boa_engine::{value::TryFromJs, Context};

use crate::{
    idl,
    stream::tmp::{self, JsFn},
};

/// dictionary [QueuingStrategy][spec] {
///   unrestricted double highWaterMark;
///   QueuingStrategySize size;
/// };
///
/// [spec]: https://streams.spec.whatwg.org/#qs
#[derive(TryFromJs)]
pub struct QueueingStrategy {
    /// **highWaterMark, of type unrestricted double**\
    /// A non-negative number indicating the high water mark of the stream using this queuing strategy.
    high_water_mark: Option<f64>,

    /// size(chunk) (non-byte streams only), of type QueuingStrategySize
    ///
    /// A function that computes and returns the finite non-negative size of the given chunk value.
    ///
    /// The result is used to determine backpressure, manifesting via the appropriate desiredSize property: either defaultController.desiredSize, byteController.desiredSize, or writer.desiredSize, depending on where the queuing strategy is being used. For readable streams, it also governs when the underlying source's pull() method is called.
    ///
    /// This function has to be idempotent and not cause side effects; very strange results can occur otherwise.
    ///
    /// For readable byte streams, this function is not used, as chunks are always measured in bytes.
    size: Option<QueuingStrategySize>,
}

/// callback QueuingStrategySize = unrestricted double (any chunk);
type QueuingStrategySize = JsFn<tmp::Todo, 1, (idl::Any,), idl::UnrestrictedDouble>;

impl QueueingStrategy {
    pub fn high_water_mark(&self) -> f64 {
        todo!()
    }
    pub fn size(&self, chunk: idl::Chunk, context: &mut Context) -> f64 {
        todo!()
    }
}
