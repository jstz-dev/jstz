use rexpect::session::{spawn_command, PtySession};
use std::process::Command;
use tempfile::TempDir;

fn jstz_cmd<'a, T>(args: T, home_dir: Option<TempDir>) -> (PtySession, TempDir)
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

#[test]
fn create_account() {
    let (mut process, _tmp_dir) = jstz_cmd(["account", "create", "foo"], None);

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
    let (mut process, _tmp_dir) = jstz_cmd(["login", "foo"], None);

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

#[test]
fn import_account() {
    let (mut process, tmp_dir) = jstz_cmd(["account", "import", "foo"], None);

    process
        .send_line("edsk4YBTjLtZgLNWKUN95unbAZ6cfq2eXhRveVt4J5oFPYHMzadpc8")
        .unwrap();

    let output = process.exp_eof().unwrap();
    assert!(output.contains("Enter the secret key of your account"));
    assert!(output.contains(
        "User foo imported with address: tz1WrJFFhUrHeozPin2KY29WJPZ9GzkmpX3Y"
    ));

    // import to the same alias should fail
    let (mut process, tmp_dir) = jstz_cmd(["account", "import", "foo"], Some(tmp_dir));

    let output = process.exp_eof().unwrap();
    assert!(output.contains("The account 'foo' already exists."));

    // import to the same alias with --force should work
    let (mut process, _) =
        jstz_cmd(["account", "import", "foo", "--force"], Some(tmp_dir));

    process
        .send_line("edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf")
        .unwrap();

    let output = process.exp_eof().unwrap();
    assert!(output.contains("Enter the secret key of your account"));
    assert!(output.contains(
        "User foo imported with address: tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6"
    ));
}

#[test]
fn import_account_empty_input() {
    let (mut process, _) = jstz_cmd(["account", "import", "foo"], None);

    process.send_line("").unwrap();

    let output = process.exp_eof().unwrap();
    assert!(output.contains("Import aborted"));
}

#[test]
fn import_account_bad_key() {
    let (mut process, _) = jstz_cmd(["account", "import", "foo"], None);

    process.send_line("aaa").unwrap();

    let output = process.exp_eof().unwrap();
    assert!(output.contains("Failed to process secret key"));
}
