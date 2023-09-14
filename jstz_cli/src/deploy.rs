use std::process::Command;

pub fn deploy(script_address: String, root_dir:String, octez_client_path: String, octez_client_setup_args: Vec<String>) {
    // Contract source
    let src = format!("{}/jstz_bridge/jstz_bridge.tz", root_dir);
    let ctez_src = format!("{}/jstz_bridge/jstz_ctez.tz", root_dir);

    // Originate ctez contract
    let bootstrap3_address = "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU";
    let init_ctez_storage = format!("(Pair \"{}\" {{ Elt \"{}\" 10000000000 }} )", bootstrap3_address, bootstrap3_address);

    let mut args: Vec<&str> = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
            [
                "originate",
                "contract",
                "jstz_ctez",
                "transferring",
                "0",
                "from",
                "bootstrap3",
                "running",
                &ctez_src,
                "--init",
                &init_ctez_storage,
                "--burn-cap",
                "999",
                "--force"
            ].iter().cloned()
        )
        .collect();
    
    let output = Command::new(&octez_client_path)
        .args(&args)
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

    args = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
            ["originate",
            "contract",
            "jstz_bridge",
            "transferring",
            "0",
            "from",
            "bootstrap3",
            "running",
            &src,
            "--init",
            &init_storage,
            "--burn-cap",
            "999",
            "--force"].iter().cloned()
        )
        .collect();

    let output = Command::new(octez_client_path.clone())
        .args(&args)
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
    let hex_string = format!("hex:[ \"{}\" ]", set_ticketer_emsg_hex);

    args = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
            ["send",
            "smart",
            "rollup",
            "message",
            &hex_string,
            "from",
            "bootstrap2"].iter().cloned()
        )
        .collect();

    Command::new(octez_client_path.clone())
        .args(&args)
        .output()
        .expect("Failed to set ticketer");

    let script_address_formatted=format!("\"{}\"", script_address);

    args = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
        [
            "transfer",
            "0",
            "from",
            "bootstrap3",
            "to",
            "jstz_bridge",
            "--entrypoint",
            "set_rollup",
            "--arg",
            &script_address_formatted,
            "--burn-cap",
            "999"].iter().cloned()
        )
        .collect();

    // Set rollup address
    Command::new(octez_client_path.clone())
        .args(&args)
        .output()
        .expect("Failed to set rollup address");

    println!("The `jstz_bridge` contract has successfully been originated and configured.");
    println!("You may now run `octez-client transfer 0 from .. to jstz_bridge ..` to communicate with `jstz_rollup` via the L1 layer.");
    println!("To upgrade the bridge, run this command again after running `make build-bridge`.");
}