use std::fmt::{self, Debug};

use crate::term::styles::{url, ErrorPrefix};

#[derive(Debug)]
pub struct UserError {
    pub message: String,
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UserError {}

pub type Error = anyhow::Error;

pub fn print(err: &Error) {
    if let Some(user_error) = err.downcast_ref::<UserError>() {
        eprintln!("{} {:#?}", ErrorPrefix, user_error);
    } else {
        eprintln!(
            "{} {:#?}\n\nIf you think this is a bug then please create an issue at {}.",
            ErrorPrefix,
            err,
            url("https://github.com/jstz-dev/jstz/issues/new/choose")
        );
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! user_error {
    ($msg:literal $(,)?) => {
        anyhow::anyhow!($crate::error::UserError {
            message: format!($msg),
        })
    };
    ($fmt:expr, $($arg:tt)*) => {
        anyhow::anyhow!($crate::error::UserError {
            message: format!($fmt, $($arg)*)
        })
    };
}

macro_rules! bail_user_error {
    ($msg:literal $(,)?) => {
        return Err($crate::error::user_error!($msg))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::user_error!($fmt, $($arg)*))
    };
}

pub(crate) use anyhow::{anyhow, bail};
pub(crate) use {bail_user_error, user_error};
