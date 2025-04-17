use rexpect::session::spawn_command;
use std::process::Command;
use tempfile::TempDir;

fn jstz_cmd() -> (Command, TempDir) {
    let tmp_dir = TempDir::new().unwrap();

    let bin_path = assert_cmd::cargo::cargo_bin("jstz");
    let mut cmd = Command::new(bin_path);
    cmd.env(
        "XDG_CONFIG_HOME",
        tmp_dir.path().to_string_lossy().to_string(),
    );
    (cmd, tmp_dir)
}

#[test]
fn create_account() {
    let (mut cmd, _tmp_dir) = jstz_cmd();
    cmd.args(["account", "create", "foo"]);

    let mut process = spawn_command(cmd, Some(30000)).unwrap();
    // empty passphrase
    process.send_line("").unwrap();

    let output = process.exp_eof().unwrap();
    assert!(output.contains("Enter the passphrase for the new account or leave empty"));
    assert!(output.contains("Generated mnemonic:"));
    assert!(output.contains("Please keep the mnemonic and the passphrase safe"));
    assert!(output.contains("User created with address: tz1"));
}

#[test]
fn login_new_account() {
    let (mut cmd, _tmp_dir) = jstz_cmd();
    cmd.args(["login", "foo"]);

    let mut process = spawn_command(cmd, Some(30000)).unwrap();
    process.send_line("y").unwrap();
    // empty passphrase
    process.send_line("").unwrap();

    let output = process.exp_eof().unwrap();
    // prompt
    assert!(output.contains("Account not found. Do you want to create it? [y/n]"));
    // after accepting 'y'
    assert!(output.contains("Account not found. Do you want to create it? yes"));
    assert!(output.contains("Enter the passphrase for the new account or leave empty"));
    assert!(output.contains("Generated mnemonic:"));
    assert!(output.contains("User created with address: tz1"));
    assert!(output.contains("Logged in to account foo with address tz1"));
}
