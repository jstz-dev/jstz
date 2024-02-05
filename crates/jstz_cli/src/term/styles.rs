use std::fmt::{self, Display};

use console::{style, StyledObject};

use crate::term::emoji;

pub struct ErrorPrefix;

impl Display for ErrorPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            style(emoji::ERROR).red(),
            style(" ERROR ").on_red().white(),
        )
    }
}

pub struct WarningPrefix;

impl Display for WarningPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            style(emoji::WARN).yellow(),
            style(" WARNING ").black().on_yellow(),
        )
    }
}

pub fn url<D>(msg: D) -> StyledObject<D> {
    style(msg).blue().bold()
}
