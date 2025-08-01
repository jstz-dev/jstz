use std::error::Error;

pub mod generate;
pub mod results;
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[allow(unused)]
mod builder;
