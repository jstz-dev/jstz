//! Conversions of Rust values to and from [`JsValue`].
//!
//! | IDL type                | Type                             |
//! |-------------------------|----------------------------------|
//! | any                     | `JsValue`                        |
//! | boolean                 | `bool`                           |
//! | byte                    | `i8`                             |
//! | octet                   | `u8`                             |
//! | short                   | `i16`                            |
//! | unsigned short          | `u16`                            |
//! | long                    | `i32`                            |
//! | unsigned long           | `u32`                            |
//! | long long               | `i64`                            |
//! | unsigned long long      | `u64`                            |
//! | unrestricted float      | `f32`                            |
//! | float                   | `f32`                            |
//! | unrestricted double     | `f64`                            |
//! | double                  | `f64`                            |
//! | USVString               | `JsString`                       |
//! | object                  | `JsObject`                       |
//! | symbol                  | `JsSymbol`                       |
//! | nullable types          | `Option<T>`                      |
//! | sequences               | `Vec<T>`                         |
//! |-------------------------|----------------------------------|

mod try_from_js;
mod try_into_js;