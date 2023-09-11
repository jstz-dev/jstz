use std::process::Command;

pub fn deploy(script_address: String, name: String) {
    let root_dir = "..";

    // Contract source
    let src = format!("{}/jstz_bridge/jstz_bridge.tz", root_dir);
    let ctez_src = format!("{}/jstz_bridge/jstz_ctez.tz", root_dir);

    // Originate ctez contract
    let bootstrap3_address = "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU";
    let init_ctez_storage = format!("(Pair \"{}\" {{ Elt \"{}\" 10000000000 }} )", bootstrap3_address, bootstrap3_address);
    let output = Command::new("client")
        .arg("originate")
        .arg("contract")
        .arg("jstz_ctez")
        .arg("transferring")
        .arg("0")
        .arg("from")
        .arg("bootstrap3")
        .arg("running")
        .arg(&ctez_src)
        .arg("--init")
        .arg(&init_ctez_storage)
        .arg("--burn-cap")
        .arg("999")
        .arg("--force")
        .output()
        .expect("Failed to originate ctez contract");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let ctez_address = output_str
        .lines()
        .filter(|line| line.contains("New contract"))
        .next()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap();

    // Originate bridge contract
    let init_storage = format!("(Pair \"{}\" None)", ctez_address);
    let output = Command::new("client")
        .arg("originate")
        .arg("contract")
        .arg("jstz_bridge")
        .arg("transferring")
        .arg("0")
        .arg("from")
        .arg("bootstrap3")
        .arg("running")
        .arg(&src)
        .arg("--init")
        .arg(&init_storage)
        .arg("--burn-cap")
        .arg("999")
        .arg("--force")
        .output()
        .expect("Failed to originate bridge contract");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let bridge_address = output_str
        .lines()
        .filter(|line| line.contains("New contract"))
        .next()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap();

    // Set ticketer
    let set_ticketer_emsg = format!("{{ \"SetTicketer\": \"{}\" }}", bridge_address);
    let set_ticketer_emsg_hex = hex::encode(set_ticketer_emsg);

    Command::new("client")
        .arg("send")
        .arg("smart")
        .arg("rollup")
        .arg("message")
        .arg(format!("hex:[ \"{}\" ]", set_ticketer_emsg_hex))
        .arg("from")
        .arg("bootstrap2")
        .output()
        .expect("Failed to set ticketer");

    // Set rollup address
    Command::new("client")
        .arg("transfer")
        .arg("0")
        .arg("from")
        .arg("bootstrap3")
        .arg("to")
        .arg("jstz_bridge")
        .arg("--entrypoint")
        .arg("set_rollup")
        .arg("--arg")
        .arg(format!("\"{}\"", script_address))
        .arg("--burn-cap")
        .arg("999")
        .output()
        .expect("Failed to set rollup address");

    println!("The `jstz_bridge` contract has successfully been originated and configured.");
    println!("You may now run `octez-client transfer 0 from .. to jstz_bridge ..` to communicate with `jstz_rollup` via the L1 layer.");
    println!("To upgrade the bridge, run this command again after running `make build-bridge`.");
}