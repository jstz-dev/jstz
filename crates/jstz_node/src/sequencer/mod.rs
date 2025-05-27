pub mod db;
pub mod queue;
pub mod worker;

#[cfg(test)]
pub mod tests {
    use axum::http::{HeaderMap, Method, Uri};
    use jstz_crypto::{public_key::PublicKey, signature::Signature};
    use jstz_proto::{
        context::account::Nonce,
        operation::{Content, Operation, RunFunction, SignedOperation},
    };
    use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};

    pub fn dummy_op() -> SignedOperation {
        SignedOperation::new(
        Signature::Ed25519(Ed25519Signature::from_base58_check("edsigtbD6jADoivxf1iho6mDYPGiVvXw4Hnurn6VzDLG1boyMmmHEAykSrUJjJpvEsHHjQNvLWfm9PdyMBfJ8CX7jSEkh3yrB6m").unwrap().into()),
        Operation {
            public_key: PublicKey::Ed25519(
                PublicKeyEd25519::from_base58_check(
                    "edpkuUXUFt2E51TkMjRarDEVWXGB4kLKoTryMDyMhNyxFCRTsPDd1K",
                )
                .unwrap()
                .into(),
            ),
            nonce: Nonce(0),
            content: Content::RunFunction(RunFunction {
                uri: Uri::from_static("http://http://"),
                method: Method::HEAD,
                headers: HeaderMap::new(),
                body: None,
                gas_limit: 0,
            }),
        },
    )
    }
}
