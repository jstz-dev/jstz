#[test]
fn test_capture_hello_world() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "jstzd"])
        .arg("jstzd")
        .output()
        .expect("Failed to run cargo run");
    let stdout =
        String::from_utf8(output.stdout).expect("Failed to convert stdout to string");
    assert!(stdout.contains("Hello, world!"));
}
