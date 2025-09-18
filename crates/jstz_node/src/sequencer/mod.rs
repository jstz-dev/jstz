pub mod db;
mod host;
pub mod inbox;
pub mod queue;
#[allow(unused)]
mod riscv_pvm;
pub mod runtime;
pub mod worker;

#[cfg(test)]
pub mod tests {
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use jstz_proto::{
        context::account::Nonce,
        operation::{Content, DeployFunction, Operation, SignedOperation},
    };
    use tezos_crypto_rs::hash::PublicKeyEd25519;

    use crate::sequencer::queue::WrappedOperation;

    pub fn dummy_signed_op() -> SignedOperation {
        let sk = SecretKey::from_base58(
            "edsk38mmuJeEfSYGiwLE1qHr16BPYKMT5Gg1mULT7dNUtg3ti4De3a",
        )
        .unwrap();
        let pk = PublicKey::Ed25519(
            PublicKeyEd25519::from_base58_check(
                "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
            )
            .unwrap()
            .into(),
        );
        let op = Operation {
            public_key: pk,
            nonce: Nonce(0),
            content: Content::DeployFunction(DeployFunction {
                account_credit: 0,
                function_code: "export default async () => {}".to_string(),
            }),
        };

        let signature = sk.sign(op.hash()).unwrap();
        SignedOperation::new(signature, op)
    }

    pub fn dummy_op() -> WrappedOperation {
        WrappedOperation::FromNode(dummy_signed_op())
    }
}
