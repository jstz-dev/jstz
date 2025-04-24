#![allow(unused)]
// Use module path attribute to import this module into integration test
// files
//
// ```
// #[path = "./utils.rs"]
// mod utils
// ```

use derive_more::{Deref, DerefMut};
use rexpect::session::{spawn_command, PtySession};
use std::process::Command;
use tempfile::TempDir;

#[derive(Deref, DerefMut)]
pub struct ProcessSession {
    #[deref]
    #[deref_mut]
    process: PtySession,
    pub tmp: TempDir,
}

pub fn jstz_cmd<'a, T>(args: T, home_dir: Option<TempDir>) -> ProcessSession
where
    T: IntoIterator<Item = &'a str>,
{
    let tmp_dir = home_dir.unwrap_or(TempDir::new().unwrap());

    let bin_path = assert_cmd::cargo::cargo_bin("jstz");
    let mut cmd = Command::new(bin_path);
    cmd.env(
        "XDG_CONFIG_HOME",
        tmp_dir.path().to_string_lossy().to_string(),
    )
    .args(args);
    let process = spawn_command(cmd, Some(30000)).unwrap();
    ProcessSession {
        process,
        tmp: tmp_dir,
    }
}

/// Runs `jstz <command>`
///
/// Config will be created in a temp directory and returned if not provided.
pub fn jstz(command: &str, config_dir: Option<TempDir>) -> ProcessSession {
    let args = command.split(" ");
    jstz_cmd(args, config_dir)
}
