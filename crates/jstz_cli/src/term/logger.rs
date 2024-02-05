use env_logger::{fmt, Builder, Env};

use std::io::{self, Write};

use crate::term::styles::{ErrorPrefix, WarningPrefix};

fn format(fmt: &mut fmt::Formatter, record: &log::Record<'_>) -> io::Result<()> {
    match record.level() {
        log::Level::Error => write!(fmt, "{} ", ErrorPrefix)?,
        log::Level::Warn => write!(fmt, "{} ", WarningPrefix)?,
        log::Level::Info | log::Level::Debug | log::Level::Trace => (),
    };

    writeln!(fmt, "{}", record.args())
}

pub fn init_logger() {
    let env = Env::default().filter("JSTZ_LOG").default_filter_or("info");

    Builder::from_env(env).format(format).init();
}
