use boa_engine::object::builtins::JsFunction;

use crate::{idl, js_aware::JsOptional};

/// dictionary [QueuingStrategy][def] {
///   unrestricted double highWaterMark;
///   QueuingStrategySize size;
/// };
///
/// [def] https://streams.spec.whatwg.org/#dictdef-queuingstrategy
///
pub struct QueuingStrategy {
    /// highWaterMark, of type unrestricted double
    ///
    ///   A non-negative number indicating the high water mark of the stream using this queuing strategy.
    high_water_mark: JsOptional<idl::UnrestrictedDouble>,
    /// size(chunk) (non-byte streams only), of type QueuingStrategySize
    ///
    /// A function that computes and returns the finite non-negative size of the given chunk value.
    ///
    /// The result is used to determine backpressure, manifesting via the appropriate desiredSize property: either defaultController.desiredSize, byteController.desiredSize, or writer.desiredSize, depending on where the queuing strategy is being used. For readable streams, it also governs when the underlying source's pull() method is called.
    ///
    /// This function has to be idempotent and not cause side effects; very strange results can occur otherwise.
    ///
    /// For readable byte streams, this function is not used, as chunks are always measured in bytes.
    size: JsOptional<QueuingStrategySize>,
}

/// callback QueuingStrategySize = unrestricted double (any chunk);
// TODO handle types?
pub type QueuingStrategySize = JsFunction;
