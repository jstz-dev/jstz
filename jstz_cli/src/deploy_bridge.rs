use crate::config::Config;
use crate::utils::handle_output;

fn extract_address(output: &[u8]) -> String {
    let output_str = String::from_utf8_lossy(output);
    output_str
        .lines()
        .filter(|line| line.contains("New contract"))
        .next()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .to_string()
}

pub fn deploy_bridge(script_address: String, cfg: &Config) {
    // Contract source
    let root_dir = cfg.get_root_dir();
    let src = format!("{}/jstz_bridge/jstz_bridge.tz", root_dir);
    let ctez_src = format!("{}/jstz_bridge/jstz_ctez.tz", root_dir);

    // Originate ctez contract
    let bootstrap3_address = "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU";
    let init_ctez_storage = format!("(Pair \"{}\" {{ Elt \"{}\" 10000000000 }} )", bootstrap3_address, bootstrap3_address);
    
    let output = cfg.octez_client_command()
        .args(
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
            ]
        )
        .output();

    handle_output(&output);

    let ctez_address = extract_address(&output.expect("Failed to originate ctez contract").stdout);

    // Originate bridge contract
    let init_storage = format!("(Pair \"{}\" None)", ctez_address);

    let output = cfg.octez_client_command()
        .args(
            [
                "originate",
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
                "--force"
            ]
        )
        .output();

    handle_output(&output);

    let bridge_address = extract_address(&output.expect("Failed to originate bridge contract").stdout);

    // Set ticketer
    let set_ticketer_emsg = format!("{{ \"SetTicketer\": \"{}\" }}", bridge_address);
    let set_ticketer_emsg_hex = hex::encode(set_ticketer_emsg);
    let hex_string = format!("hex:[ \"{}\" ]", set_ticketer_emsg_hex);

    let output = cfg.octez_client_command()
        .args(
            [
                "send",
                "smart",
                "rollup",
                "message",
                &hex_string,
                "from",
                "bootstrap2"
            ]
        )
        .output();
    
    handle_output(&output);

    let script_address_formatted=format!("\"{}\"", script_address);

    // Set rollup address
    let output = cfg.octez_client_command()
        .args(
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
                "999"
            ]
        )
        .output();

    handle_output(&output);

    println!("The `jstz_bridge` contract has successfully been originated and configured.");
    println!("You may now run `octez-client transfer 0 from .. to jstz_bridge ..` to communicate with `jstz_rollup` via the L1 layer.");
    println!("To upgrade the bridge, run this command again after running `make build-bridge`.");
}