use http::{HeaderMap, Method, Uri};
use jstz_crypto::{
    hash::Hash, keypair_from_mnemonic, public_key::PublicKey,
    public_key_hash::PublicKeyHash, secret_key::SecretKey,
    smart_function_hash::SmartFunctionHash,
};
use jstz_kernel::inbox::{encode_parsed_inbox_message, LevelInfo, ParsedInboxMessage};
use jstz_proto::{
    context::account::{Address, Nonce},
    operation::{
        internal::{Deposit, InboxId},
        Content, DeployFunction, Operation, RunFunction, SignedOperation,
    },
    runtime::ParsedCode,
    HttpBody,
};
#[cfg(feature = "v2_runtime")]
use jstz_proto::{operation::OracleResponse, runtime::v2::fetch::http::Response};
use std::error::Error;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    types::SmartRollupAddress,
    utils::inbox::file::{InboxFile, Message},
};
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DEFAULT_GAS_LIMIT: u32 = 100_000;
const MNEMONIC: &str =
    "donate kidney style loyal nose core inflict cup symptom speed giant polar";

pub struct Account {
    pub nonce: Nonce,
    pub sk: SecretKey,
    pub pk: PublicKey,
    pub address: Address,
}

pub struct InboxBuilder {
    messages: Vec<Message>,
    rollup_address: SmartRollupAddress,
    next_account_id: usize,
    ticketer_address: ContractKt1Hash,
    next_level: u64,
    #[cfg(feature = "v2_runtime")]
    next_oracle_request_id: u64,
    #[cfg(feature = "v2_runtime")]
    oracle_signer: Option<Account>,
}

impl InboxBuilder {
    pub fn new(
        rollup_address: SmartRollupAddress,
        ticketer_address: ContractKt1Hash,
        #[cfg(feature = "v2_runtime")] oracle_signer: Option<Account>,
    ) -> Self {
        let mut builder = Self {
            rollup_address,
            messages: Vec::new(),
            next_account_id: 0,
            ticketer_address,
            next_level: 0,
            #[cfg(feature = "v2_runtime")]
            next_oracle_request_id: 0,
            #[cfg(feature = "v2_runtime")]
            oracle_signer,
        };
        builder.bump_level().expect("should set up level 0");
        builder
    }

    pub fn build(self) -> InboxFile {
        InboxFile(vec![self.messages])
    }

    pub fn create_accounts(&mut self, count: usize) -> Result<Vec<Account>> {
        let mut accounts = vec![];
        for i in self.next_account_id..count + self.next_account_id {
            let (pk, sk) = keypair_from_mnemonic(MNEMONIC, &i.to_string())?;
            let account = Account {
                address: Address::from_base58(&pk.hash())?,
                sk,
                pk,
                nonce: Default::default(),
            };
            accounts.push(account);
        }
        self.next_account_id += count;
        Ok(accounts)
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    fn generate_external_message(
        &self,
        signer: &Account,
        content: Content,
    ) -> Result<Message> {
        let op = Operation {
            public_key: signer.pk.clone(),
            nonce: signer.nonce,
            content,
        };

        let hash = op.hash();
        let signed_op = SignedOperation::new(signer.sk.sign(hash)?, op);

        let bytes = encode_parsed_inbox_message(
            &ParsedInboxMessage::JstzMessage(jstz_kernel::inbox::Message::External(
                signed_op,
            )),
            &self.ticketer_address,
            &self.rollup_address,
        )?;
        Ok(Message::Raw(bytes))
    }

    pub fn deploy_function(
        &mut self,
        account: &mut Account,
        code: ParsedCode,
        account_credit: u64,
    ) -> Result<Address> {
        // TODO: JSTZ-849 somehow reuse the logic in proto
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
        body: HttpBody,
    ) -> Result<()> {
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

    pub fn deposit_from_l1(
        &mut self,
        account: &Account,
        amount_mutez: u64,
    ) -> Result<()> {
        let bytes = encode_parsed_inbox_message(
            &ParsedInboxMessage::JstzMessage(jstz_kernel::inbox::Message::Internal(
                jstz_proto::operation::InternalOperation::Deposit(Deposit {
                    inbox_id: InboxId {
                        l1_level: (self.next_level - 1) as u32,
                        // message id does not matter for now
                        l1_message_id: 0,
                    },
                    amount: amount_mutez,
                    receiver: account.address.clone(),
                    source: PublicKeyHash::from_base58(
                        "tz1W8rEphWEjMcD1HsxEhsBFocfMeGsW7Qxg",
                    )
                    .expect("the constant source address should be parsable"),
                }),
            )),
            &self.ticketer_address,
            &self.rollup_address,
        )?;
        self.messages.push(Message::Raw(bytes));
        Ok(())
    }

    pub fn withdraw(
        &mut self,
        account: &mut Account,
        receiver: &Address,
        amount_mutez: u64,
    ) -> Result<()> {
        let uri = Uri::from_static("jstz://jstz/withdraw");
        let withdraw = jstz_proto::executor::withdraw::Withdrawal {
            amount: amount_mutez,
            receiver: receiver.clone(),
        };
        let json_data = serde_json::to_vec(&withdraw)?;
        self.run_function(
            account,
            uri,
            Method::POST,
            HeaderMap::default(),
            HttpBody(Some(json_data)),
        )
    }

    #[cfg(feature = "v2_runtime")]
    pub fn create_oracle_response(&mut self, response: Response) -> Result<()> {
        if self.oracle_signer.is_none() {
            return Err(
                "cannot build oracle response: oracle signer is not provided".into(),
            );
        }

        let signer = self.oracle_signer.as_ref().unwrap();
        let oracle_response = OracleResponse {
            request_id: self.next_oracle_request_id,
            response,
        };
        let message = self.generate_external_message(
            signer,
            Content::OracleResponse(oracle_response),
        )?;
        self.messages.push(message);

        let signer = self.oracle_signer.as_mut().unwrap();
        signer.nonce = signer.nonce.next();
        self.next_oracle_request_id += 1;
        Ok(())
    }

    pub fn bump_level(&mut self) -> Result<()> {
        if self.next_level > 0 {
            self.messages.push(Message::Raw(encode_parsed_inbox_message(
                &ParsedInboxMessage::LevelInfo(LevelInfo::End),
                &self.ticketer_address,
                &self.rollup_address,
            )?));
        }

        self.messages.push(Message::Raw(encode_parsed_inbox_message(
            &ParsedInboxMessage::LevelInfo(LevelInfo::Start),
            &self.ticketer_address,
            &self.rollup_address,
        )?));
        self.next_level += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use http::{HeaderMap, Method, Uri};
    use jstz_core::BinEncodable;
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use jstz_kernel::inbox::RollupType;
    use jstz_proto::{
        context::account::{Address, Nonce},
        executor::withdraw::Withdrawal,
        operation::{Content, DeployFunction, RunFunction, SignedOperation},
        runtime::ParsedCode,
        HttpBody,
    };
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup::{
        inbox::{ExternalMessageFrame, InboxMessage, InternalInboxMessage},
        michelson::{MichelsonOr, MichelsonUnit},
        types::SmartRollupAddress,
        utils::inbox::file::Message,
    };

    use super::InboxBuilder;

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

    fn ticketer() -> ContractKt1Hash {
        ContractKt1Hash::from_base58_check("KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5")
            .unwrap()
    }

    #[test]
    fn create_accounts() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(
            rollup_address,
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        let accounts = builder.create_accounts(10).unwrap();
        let mut addresses = accounts.iter().map(|v| v.pk.hash()).collect::<HashSet<_>>();
        assert_eq!(addresses.len(), 10);

        let accounts = builder.create_accounts(10).unwrap();
        for account in accounts {
            assert!(addresses.insert(account.pk.hash()));
        }
        assert_eq!(addresses.len(), 20);
    }

    #[test]
    fn run_function() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        builder
            .run_function(
                &mut default_account(),
                Uri::try_from("jstz://foobar/transfer".to_string()).unwrap(),
                Method::GET,
                HeaderMap::new(),
                HttpBody::empty(),
            )
            .unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(b).unwrap();
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
        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        builder
            .deploy_function(&mut default_account(), ParsedCode("code".to_string()), 123)
            .unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(b).unwrap();
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
        let builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        let message = builder
            .generate_external_message(&default_account(), content.clone())
            .unwrap();

        match message {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(b).unwrap();
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

    #[test]
    fn deposit_from_l1() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        let account = builder.create_accounts(1).unwrap().pop().unwrap();

        builder.deposit_from_l1(&account, 1).unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<RollupType>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::Internal(InternalInboxMessage::Transfer(transfer)) => {
                        assert_eq!(transfer.destination, builder.rollup_address);
                        assert_eq!(transfer.sender, builder.ticketer_address);
                        assert!(matches!(transfer.payload, MichelsonOr::Left(_)));
                    }
                    _ => panic!("should be internal message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }

    #[test]
    fn withdraw() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        let mut accounts = builder.create_accounts(2).unwrap();
        let mut account = accounts.pop().unwrap();
        let receiver = accounts.pop().unwrap();
        builder
            .withdraw(&mut account, &receiver.address, 10000)
            .unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(b).unwrap();
                        match v {
                            ExternalMessageFrame::Targetted { address, contents } => {
                                assert_eq!(address, rollup_address);
                                let op = SignedOperation::decode(contents).unwrap();
                                match op.content() {
                                    Content::RunFunction(RunFunction {
                                        uri,
                                        method,
                                        headers: _,
                                        body,
                                        gas_limit: _,
                                    }) => {
                                        assert_eq!(
                                            uri.to_string(),
                                            "jstz://jstz/withdraw"
                                        );
                                        assert_eq!(method, Method::POST);
                                        let withdrawal: Withdrawal =
                                            serde_json::from_slice(
                                                body.as_ref().unwrap(),
                                            )
                                            .unwrap();
                                        assert_eq!(
                                            &withdrawal.receiver,
                                            &receiver.address
                                        );
                                        assert_eq!(withdrawal.amount, 10000);
                                    }
                                    _ => panic!("should be run function"),
                                }
                            }
                        }
                    }
                    _ => panic!("should be external message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }

    #[cfg(feature = "v2_runtime")]
    #[test]
    fn create_oracle_response() {
        use jstz_proto::runtime::v2::fetch::http::{Body, Response};

        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let response = Response {
            status: 400,
            status_text: "foobar".to_string(),
            headers: vec![],
            body: Body::zero_capacity(),
        };
        let mut builder = InboxBuilder::new(rollup_address.clone(), ticketer(), None);
        assert_eq!(
            builder
                .create_oracle_response(response.clone())
                .unwrap_err()
                .to_string(),
            "cannot build oracle response: oracle signer is not provided"
        );

        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            ticketer(),
            Some(default_account()),
        );
        builder.create_oracle_response(response).unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::External(b) => {
                        let v = ExternalMessageFrame::parse(b).unwrap();
                        match v {
                            ExternalMessageFrame::Targetted { address, contents } => {
                                assert_eq!(address, rollup_address);
                                let op = SignedOperation::decode(contents).unwrap();
                                match op.content() {
                                    Content::OracleResponse(res) => {
                                        assert_eq!(res.request_id, 0);
                                        assert_eq!(res.response.status_text, "foobar");
                                        assert_eq!(res.response.status, 400);
                                    }
                                    _ => panic!("should be oracle response"),
                                };
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
    fn bump_level() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(
            rollup_address,
            ticketer(),
            #[cfg(feature = "v2_runtime")]
            None,
        );
        // 1 message: start of level 0
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                matches!(
                    inbox_msg,
                    InboxMessage::Internal(InternalInboxMessage::StartOfLevel)
                );
            }
            _ => panic!("should be raw message"),
        }

        // there should be one end of level message for level 0 and
        // one start of level message for level 1
        builder.bump_level().unwrap();
        assert_eq!(builder.messages.len(), 2);
        match builder.messages.first().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(raw).unwrap();
                matches!(
                    inbox_msg,
                    InboxMessage::Internal(InternalInboxMessage::EndOfLevel)
                );
            }
            _ => panic!("should be raw message"),
        }
        match builder.messages.last().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(raw).unwrap();
                matches!(
                    inbox_msg,
                    InboxMessage::Internal(InternalInboxMessage::StartOfLevel)
                );
            }
            _ => panic!("should be raw message"),
        }
    }
}
