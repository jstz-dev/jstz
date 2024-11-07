mod utils;

use jstzd::task::Task;
use octez::r#async::{
    endpoint::Endpoint,
    protocol::{
        BootstrapAccount, BootstrapContract, BootstrapSmartRollup,
        ProtocolParameterBuilder, ReadWritable, SmartRollupPvmKind,
    },
};
use utils::{
    activate_alpha, create_client, import_activator, import_bootstrap_keys,
    spawn_octez_node,
};

#[tokio::test(flavor = "multi_thread")]
async fn protocol_parameters() {
    let mut param_builder = ProtocolParameterBuilder::new();
    // showing that it's fine as long as at least one account has sufficient balance
    let bootstrap_accounts = [
        (
            "tz1Ke5At2oVQGyjtU325kUEANe1XGmJBHj2Y",
            "edpktkhoky4f5kqm2EVwYrMBq5rY9sLYdpFgXixQDWifuBHjhuVuNN",
            6000000000,
        ),
        (
            "tz1L9RF6ybHfGuced5VDv31pn2zxrmxDnJaS",
            "edpkughHYKvKWBEZMjbXnd7VhrqNbNvN8jjgC83XGPytEuTGZSgjBi",
            1,
        ),
    ];
    let bootstrap_contract_address = "KT1DjrMdtHteERJHsKxKjEhvmpz95owLLMbM";
    let bootstrap_rollup_address = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
    param_builder.set_bootstrap_accounts(
        bootstrap_accounts
            .iter()
            .map(|&(_, key, amount)| BootstrapAccount::new(key, amount).unwrap()),
    );
    param_builder.set_bootstrap_contracts([BootstrapContract::new(
            serde_json::json!({"code":[{"prim":"parameter","args":[{"prim":"unit","annots":["%entrypoint_1"]}]},{"prim":"storage","args":[{"prim":"int"}]},{"prim":"code","args":[[{"prim":"CDR"},{"prim":"NIL","args":[{"prim":"operation"}]},{"prim":"PAIR"}]]}],"storage":{"int":"1"}}),
            2000000,
            Some(bootstrap_contract_address)
        )
        .unwrap()]);
    param_builder.set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
            bootstrap_rollup_address,
            SmartRollupPvmKind::Wasm,
            "23212f7573722f62696e2f656e762073680a6578706f7274204b45524e454c3d22303036313733366430313030303030303031323830373630303337663766376630313766363030323766376630313766363030353766376637663766376630313766363030313766303036303031376630313766363030323766376630303630303030303032363130333131373336643631373237343566373236663663366337353730356636333666373236353061373236353631363435663639366537303735373430303030313137333664363137323734356637323666366336633735373035663633366637323635306337373732363937343635356636663735373437303735373430303031313137333664363137323734356637323666366336633735373035663633366637323635306237333734366637323635356637373732363937343635303030323033303530343033303430353036303530333031303030313037313430323033366436353664303230303061366236353732366536353663356637323735366530303036306161343031303432613031303237663431666130303266303130303231303132303030326630313030323130323230303132303032343730343430343165343030343131323431303034316534303034313030313030323161306230623038303032303030343163343030366230623530303130353766343166653030326430303030323130333431666330303266303130303231303232303030326430303030323130343230303032663031303032313035323030313130303432313036323030343230303334363034343032303030343130313661323030313431303136623130303131613035323030353230303234363034343032303030343130373661323030363130303131613062306230623164303130313766343164633031343138343032343139303163313030303231303034313834303232303030313030353431383430323130303330623062333830353030343165343030306231323266366236353732366536353663326636353665373632663732363536323666366637343030343166383030306230323030303130303431666130303062303230303032303034316663303030623032303030303030343166653030306230313031220a",
            serde_json::json!({ "prim": "bytes" }),
        )
        .unwrap()]);
    let param_file = param_builder.build().unwrap();

    let mut octez_node = spawn_octez_node().await;
    let octez_client = create_client(octez_node.rpc_endpoint());
    import_bootstrap_keys(&octez_client).await;
    import_activator(&octez_client).await;
    activate_alpha(&octez_client, Some(param_file.path())).await;

    for (address, _, amount) in bootstrap_accounts {
        check_bootstrap_contract(octez_node.rpc_endpoint(), address, amount).await;
    }

    check_bootstrap_contract(
        octez_node.rpc_endpoint(),
        bootstrap_contract_address,
        2000000,
    )
    .await;

    check_bootstrap_rollup(
        octez_node.rpc_endpoint(),
        bootstrap_rollup_address,
        SmartRollupPvmKind::Wasm,
    )
    .await;

    octez_node.kill().await.unwrap();
}

async fn check_bootstrap_contract(
    endpoint: &Endpoint,
    address: &str,
    expected_balance: u64,
) {
    let balance_str = reqwest::get(format!(
        "{}/chains/main/blocks/head/context/contracts/{}/full_balance",
        endpoint, address
    ))
    .await
    .unwrap_or_else(|_| panic!("should be able to get bootstrap contract {}", address))
    .text()
    .await
    .expect("should be a valid string")
    .trim()
    .replace("\"", "");
    assert_eq!(
        balance_str,
        expected_balance.to_string(),
        "address {} has {} mutez in full balance but should have {}",
        address,
        balance_str,
        expected_balance
    );
}

async fn check_bootstrap_rollup(
    endpoint: &Endpoint,
    address: &str,
    pvm_kind: SmartRollupPvmKind,
) {
    let kind = reqwest::get(format!(
        "{}/chains/main/blocks/head/context/smart_rollups/smart_rollup/{}/kind",
        endpoint, address
    ))
    .await
    .expect("should be able to get bootstrap rollup")
    .json::<SmartRollupPvmKind>()
    .await
    .expect("should be a valid pvm_kind string");
    assert_eq!(kind, pvm_kind);
}
