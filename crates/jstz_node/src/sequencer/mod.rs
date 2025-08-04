pub mod db;
mod host;
pub mod inbox;
pub mod queue;
pub mod runtime;
pub mod worker;

#[cfg(test)]
pub mod tests {
    use jstz_crypto::{public_key::PublicKey, signature::Signature};
    use jstz_proto::{
        context::account::Nonce,
        operation::{Content, DeployFunction, Operation, SignedOperation},
        runtime::ParsedCode,
    };
    use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};

    use jstz_kernel::inbox::Message;

    use jstz_kernel::inbox::ParsedInboxMessage;

    pub fn dummy_op() -> ParsedInboxMessage {
        let inner = SignedOperation::new(
        Signature::Ed25519(Ed25519Signature::from_base58_check("edsigtkikkYx71PqeJigBom8sAf8ajRqynraWUFxej5XcbVFSzga6gHYz7whJTFJhZZRywQfXKUjSQeXHPikHJt114hUTEXJzED").unwrap().into()),
         Operation {
            public_key: PublicKey::Ed25519(
                PublicKeyEd25519::from_base58_check(
                    "edpkuXD2CqRpWoTT8p4exrMPQYR2NqsYH3jTMeJMijHdgQqkMkzvnz",
                )
                .unwrap()
                .into(),
            ),
            nonce: Nonce(0),
            content: Content::DeployFunction(DeployFunction {
                account_credit: 0,
                function_code: ParsedCode("1\n".to_string())
            }),
        },
    );

        ParsedInboxMessage::JstzMessage(Message::External(inner))
    }
}
