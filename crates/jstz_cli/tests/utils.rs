use rexpect::session::{spawn_command, PtySession};
use std::process::Command;
use tempfile::TempDir;

#[allow(unused)]
pub fn jstz_cmd<'a, T>(args: T, home_dir: Option<TempDir>) -> (PtySession, TempDir)
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
    (process, tmp_dir)
}
