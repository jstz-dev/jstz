use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    context::account::{Account, Addressable},
    error::Result,
    operation::DeployFunction,
    receipt::DeployFunctionReceipt,
    runtime::ParsedCode,
    Error,
};

pub fn deploy_smart_function(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &impl Addressable,
    function_code: ParsedCode,
    account_credit: u64,
) -> Result<SmartFunctionHash> {
    let address =
        Account::create_smart_function(hrt, tx, source, account_credit, function_code)?;
    Account::sub_balance(hrt, tx, source, account_credit)?;
    Ok(address)
}

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &impl Addressable,
    deployment: DeployFunction,
) -> Result<DeployFunctionReceipt> {
    let DeployFunction {
        function_code,
        account_credit,
    } = deployment;

    // SAFETY: Smart function creation and sub_balance must be atomic
    tx.begin();
    match deploy_smart_function(hrt, tx, source, function_code, account_credit) {
        Ok(address) => {
            tx.commit(hrt)?;
            debug_msg!(hrt, "[📜] Smart function deployed: {}\n", address);
            Ok(DeployFunctionReceipt { address })
        }
        Err(err @ Error::AccountExists) => {
            tx.rollback()?;
            debug_msg!(hrt, "[📜] Smart function was already deployed\n");
            Err(err)
        }
        Err(err) => {
            tx.rollback()?;
            debug_msg!(hrt, "[📜] Smart function deployment failed. \n");
            Err(err)
        }
    }
}

#[cfg(test)]
mod test {
    use jstz_mock::host::JstzMockHost;

    use crate::{context::account::Address, executor::smart_function};

    use super::*;

    #[test]
    fn execute_deploy_deploys_smart_function_with_kt1_account1() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let hrt = host.rt();
        tx.begin();

        let deployment = DeployFunction {
            function_code: "export default () => {}".to_string().try_into().unwrap(),
            account_credit: 0,
        };
        let result = smart_function::deploy::execute(hrt, &mut tx, &source, deployment);
        assert!(result.is_ok());
        let receipt = result;
        assert!(receipt.is_ok());
    }

    #[test]
    fn execute_deploy_deploys_smart_function_with_insufficient_funds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let hrt = host.rt();
        tx.begin();
        Account::set_balance(hrt, &mut tx, &source, 0).unwrap();

        let deployment = DeployFunction {
            function_code: "export default () => {}".to_string().try_into().unwrap(),
            account_credit: 10000,
        };
        let result = smart_function::deploy::execute(hrt, &mut tx, &source, deployment);
        assert!(result.is_err_and(|e| { e.to_string().contains("InsufficientFunds") }));
    }
}
