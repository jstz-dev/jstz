use http::{HeaderMap, Method, Uri};
use jstz_core::BinEncodable;
use jstz_crypto::{
    hash::Hash, keypair_from_mnemonic, public_key::PublicKey, secret_key::SecretKey,
    smart_function_hash::SmartFunctionHash,
};
use jstz_proto::{
    context::account::{Address, Nonce},
    operation::{Content, DeployFunction, Operation, RunFunction, SignedOperation},
    runtime::ParsedCode,
};
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::{
    inbox::{ExternalMessageFrame, InboxMessage},
    michelson::MichelsonUnit,
    types::SmartRollupAddress,
    utils::inbox::file::{InboxFile, Message},
};

// tag + 20 byte address
const EXTERNAL_FRAME_SIZE: usize = 21;
const DEFAULT_GAS_LIMIT: u32 = 100_000;
const MNEMONIC: &str =
    "donate kidney style loyal nose core inflict cup symptom speed giant polar";

pub struct Account {
    nonce: Nonce,
    sk: SecretKey,
    pk: PublicKey,
    pub address: Address,
}

pub struct InboxBuilder {
    messages: Vec<Message>,
    rollup_address: SmartRollupAddress,
}

impl InboxBuilder {
    pub fn new(rollup_address: SmartRollupAddress) -> Self {
        Self {
            rollup_address,
            messages: Vec::new(),
        }
    }

    pub fn build(self) -> InboxFile {
        InboxFile(vec![self.messages])
    }

    pub fn create_accounts(count: usize) -> crate::Result<Vec<Account>> {
        let mut accounts = vec![];
        for i in 0..count {
            let (pk, sk) = keypair_from_mnemonic(MNEMONIC, &i.to_string())?;
            let account = Account {
                address: Address::from_base58(&pk.hash())?,
                sk,
                pk,
                nonce: Default::default(),
            };
            accounts.push(account);
        }
        Ok(accounts)
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    fn generate_external_message(
        &self,
        signer: &Account,
        content: Content,
    ) -> crate::Result<Message> {
        let op = Operation {
            public_key: signer.pk.clone(),
            nonce: signer.nonce,
            content,
        };

        let hash = op.hash();
        let signed_op = SignedOperation::new(signer.sk.sign(hash)?, op);

        let bytes = signed_op.encode()?;
        let mut external = Vec::with_capacity(bytes.len() + EXTERNAL_FRAME_SIZE);

        let frame = ExternalMessageFrame::Targetted {
            contents: bytes,
            address: self.rollup_address.clone(),
        };
        frame.bin_write(&mut external)?;

        let inbox_message = InboxMessage::External::<MichelsonUnit>(&external);
        let mut bytes = Vec::new();
        inbox_message.serialize(&mut bytes)?;
        Ok(Message::Raw(bytes))
    }

    pub fn deploy_function(
        &mut self,
        account: &mut Account,
        code: ParsedCode,
        account_credit: u64,
    ) -> crate::Result<Address> {
        let address = Address::SmartFunction(SmartFunctionHash::digest(
            format!("{}{}{}", &account.address, code, account.nonce.next()).as_bytes(),
        )?);

        let content = Content::DeployFunction(DeployFunction {
            function_code: code,
            account_credit,
        });

        let message = self.generate_external_message(account, content)?;
        self.messages.push(message);
        account.nonce = account.nonce.next();

        Ok(address)
    }

    pub fn run_function(
        &mut self,
        account: &mut Account,
        uri: Uri,
        method: Method,
        headers: HeaderMap,
        body: Option<Vec<u8>>,
    ) -> crate::Result<()> {
        let content = Content::RunFunction(RunFunction {
            uri,
            method,
            headers,
            body,
            gas_limit: DEFAULT_GAS_LIMIT.try_into()?,
        });

        let message = self.generate_external_message(account, content)?;
        self.messages.push(message);
        account.nonce = account.nonce.next();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use http::{HeaderMap, Method, Uri};
    use jstz_core::BinEncodable;
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use jstz_proto::{
        context::account::{Address, Nonce},
        operation::{Content, DeployFunction, SignedOperation},
        runtime::ParsedCode,
    };
    use tezos_smart_rollup::{
        inbox::{ExternalMessageFrame, InboxMessage},
        michelson::MichelsonUnit,
        types::SmartRollupAddress,
        utils::inbox::file::Message,
    };

    use crate::builder::InboxBuilder;

    fn default_account() -> super::Account {
        super::Account {
            nonce: Nonce(0),
            sk: SecretKey::from_base58(
                "edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf",
            )
            .unwrap(),
            pk: PublicKey::from_base58(
                "edpktpcAZ3d8Yy1EZUF1yX4xFgLq5sJ7cL9aVhp7aV12y89RXThE3N",
            )
            .unwrap(),
            address: Address::from_base58("tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6")
                .unwrap(),
        }
    }

    #[test]
    fn create_accounts() {
        let accounts = InboxBuilder::create_accounts(10).unwrap();
        let addresses = accounts
            .iter()
            .map(|v| v.address.clone())
            .collect::<HashSet<_>>();
        assert_eq!(addresses.len(), 10);
    }

    #[test]
    fn run_function() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(rollup_address.clone());
        builder
            .run_function(
                &mut default_account(),
                Uri::try_from(format!("jstz://foobar/transfer")).unwrap(),
                Method::GET,
                HeaderMap::new(),
                None,
            )
            .unwrap();
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.first().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(&b).unwrap();
                        match v {
                            ExternalMessageFrame::Targetted { address, contents } => {
                                assert_eq!(address, rollup_address);
                                let op = SignedOperation::decode(contents).unwrap();
                                matches!(op.content(), Content::RunFunction(_));
                            }
                        }
                    }
                    _ => panic!("should be external message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }

    #[test]
    fn deploy_function() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(rollup_address.clone());
        builder
            .deploy_function(&mut default_account(), ParsedCode("code".to_string()), 123)
            .unwrap();
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.first().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(&b).unwrap();
                        match v {
                            ExternalMessageFrame::Targetted { address, contents } => {
                                assert_eq!(address, rollup_address);
                                let op = SignedOperation::decode(contents).unwrap();
                                matches!(op.content(), Content::DeployFunction(_));
                            }
                        }
                    }
                    _ => panic!("should be external message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }

    #[test]
    fn generate_external_message() {
        let content = Content::DeployFunction(DeployFunction {
            function_code: ParsedCode("foo".to_string()),
            account_credit: 123,
        });

        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let builder = InboxBuilder::new(rollup_address.clone());
        let message = builder
            .generate_external_message(&default_account(), content.clone())
            .unwrap();

        match message {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(&b).unwrap();
                        match v {
                            ExternalMessageFrame::Targetted { address, contents } => {
                                assert_eq!(address, rollup_address);
                                let op = SignedOperation::decode(contents).unwrap();
                                assert_eq!(op.content, content);
                            }
                        }
                    }
                    _ => panic!("should be external message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }
}
