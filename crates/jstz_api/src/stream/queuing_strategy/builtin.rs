use boa_engine::{
    js_string, object::ErasedObject, property::Attribute, value::TryFromJs, Context,
    JsArgs, JsData, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    js_fn::JsCallableWithoutThis,
    native::{Accessor, ClassBuilder, NativeClass},
};

use crate::{
    idl,
    stream::{
        queuing_strategy::size::{
            ByteLengthQueuingStrategySizeAlgorithm, CountQueuingStrategySizeAlgorithm,
        },
        tmp::get_jsobject_property,
    },
};

/// [Streams Standard - § 7.1.][https://streams.spec.whatwg.org/#qs-api]
/// > ```
/// > dictionary QueuingStrategyInit {
/// >   required unrestricted double highWaterMark;
/// > };
/// > ```
pub struct QueuingStrategyInit {
    pub high_water_mark: idl::UnrestrictedDouble,
}

impl TryFromJs for QueuingStrategyInit {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let this = value.to_object(context)?;
        let high_water_mark: idl::UnrestrictedDouble =
            get_jsobject_property(&this, "highWaterMark", context)?
                .try_js_into::<Option<idl::UnrestrictedDouble>>(context)?
                .ok_or_else(|| {
                    JsNativeError::typ()
                        .with_message("Missing required highWaterMark property")
                })?;
        // TODO: .try_js_into::<Option<idl::UnrestrictedDouble>>(context) is slightly wrong. "arandomstring" should map to "NaN"
        Ok(QueuingStrategyInit { high_water_mark })
    }
}

/// Streams Standard - § 7.3. The CountQueuingStrategy class][https://streams.spec.whatwg.org/#cqs-class]
/// > A common queuing strategy when dealing with streams of generic objects is to simply count the number of chunks that have been accumulated so far, waiting until this number reaches a specified high-water mark. As such, this strategy is also provided out of the box.
///
/// [Streams Standard - § 7.3.1.][https://streams.spec.whatwg.org/#countqueuingstrategy]
/// > ```
/// > [Exposed=*]
/// > interface CountQueuingStrategy {
/// >   constructor(QueuingStrategyInit init);
/// >
/// >   readonly attribute unrestricted double highWaterMark;
/// >   readonly attribute Function size;
/// > };
/// > ```
///
#[derive(Trace, Finalize, JsData)]
pub struct CountQueuingStrategy {
    pub high_water_mark: idl::UnrestrictedDouble,
}

impl CountQueuingStrategy {
    pub fn size_algorithm(&self) -> CountQueuingStrategySizeAlgorithm {
        CountQueuingStrategySizeAlgorithm::ReturnOne
    }
}

#[derive(Trace, Finalize, JsData)]
pub struct ByteLengthQueuingStrategy {
    pub high_water_mark: idl::UnrestrictedDouble,
}

impl ByteLengthQueuingStrategy {
    pub fn size_algorithm(&self) -> ByteLengthQueuingStrategySizeAlgorithm {
        ByteLengthQueuingStrategySizeAlgorithm::ReturnByteLengthOfChunk
    }
}

macro_rules! impl_for_builtin_queuing_strategy_struct {
    ($struct_name: ident, $struct_name_as_str: expr) => {
        impl $struct_name {
            pub fn new(init: QueuingStrategyInit) -> Self {
                $struct_name {
                    high_water_mark: init.high_water_mark,
                }
            }

            pub fn try_from_js(
                value: &JsValue,
            ) -> JsResult<GcRefMut<ErasedObject, Self>> {
                value
                    .as_object()
                    .and_then(|obj| obj.downcast_mut::<Self>())
                    .ok_or_else(|| {
                        JsNativeError::typ()
                            .with_message(format!(
                                "Failed to convert js value into Rust type `{}`",
                                $struct_name_as_str
                            ))
                            .into()
                    })
            }
        }
    };
}

impl_for_builtin_queuing_strategy_struct!(CountQueuingStrategy, "CountQueuingStrategy");
impl_for_builtin_queuing_strategy_struct!(
    ByteLengthQueuingStrategy,
    "ByteLengthQueuingStrategy"
);

pub struct CountQueuingStrategyClass {}
pub struct ByteLengthQueuingStrategyClass {}

macro_rules! define_high_water_mark_accessor_for_builtin_queuing_strategy_class {
    ($class_name : ident) => {
        impl $class_name {
            fn high_water_mark(context: &mut Context) -> Accessor {
                accessor!(
                    context,
                    CountQueuingStrategy,
                    "highWaterMark",
                    get:((strategy, _context) => Ok(strategy.high_water_mark.into()))
                )
            }
        }
    }
}

define_high_water_mark_accessor_for_builtin_queuing_strategy_class!(
    CountQueuingStrategyClass
);
define_high_water_mark_accessor_for_builtin_queuing_strategy_class!(
    ByteLengthQueuingStrategyClass
);

impl CountQueuingStrategyClass {
    fn size(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        Ok(CountQueuingStrategySizeAlgorithm::RETURN_VALUE.into())
    }
}

impl ByteLengthQueuingStrategyClass {
    fn size(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let chunk = args.get_or_undefined(0);
        ByteLengthQueuingStrategySizeAlgorithm::ReturnByteLengthOfChunk
            .call_without_this((chunk.clone(),), context)
            .map(Into::into)
    }
}

macro_rules! define_builtin_queuing_strategy_class(
    ($struct_name:ident, $size_algorithm_type: ty, $struct_name_as_str: expr, $class_name:ident) => {
        impl NativeClass for $class_name {
            type Instance = $struct_name;

            const NAME: &'static str = $struct_name_as_str;

            fn data_constructor(
                _target: &JsValue,
                args: &[JsValue],
                context: &mut Context,
            ) -> JsResult<Self::Instance> {
                let init: QueuingStrategyInit = args
                    .first()
                    .ok_or_else(|| {
                        JsNativeError::typ()
                            .with_message(format!("{} constructor: At least 1 argument required, but only 0 passed", $struct_name_as_str))
                    })?
                    .try_js_into(context)?;
                Ok($struct_name::new(init))
            }

            fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
                let high_water_mark = Self::high_water_mark(class.context());

                class
                    .accessor(
                        js_string!("highWaterMark"),
                        high_water_mark,
                        Attribute::READONLY | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
                    )
                    .method(
                        js_string!("size"),
                        1,
                        NativeFunction::from_fn_ptr(Self::size),
                    );
                Ok(())
            }
        }
    }
);

define_builtin_queuing_strategy_class!(
    CountQueuingStrategy,
    CountQueuingStrategySizeAlgorithm,
    "CountQueuingStrategy",
    CountQueuingStrategyClass
);

define_builtin_queuing_strategy_class!(
    ByteLengthQueuingStrategy,
    ByteLengthQueuingStrategySizeAlgorithm,
    "ByteLengthQueuingStrategy",
    ByteLengthQueuingStrategyClass
);
