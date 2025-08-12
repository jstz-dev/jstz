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
use std::error::Error;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::{
    inbox::{ExternalMessageFrame, InboxMessage, InternalInboxMessage, Transfer},
    michelson::{
        ticket::{FA2_1Ticket, Ticket},
        Michelson, MichelsonContract, MichelsonNat, MichelsonOption, MichelsonOr,
        MichelsonPair, MichelsonUnit,
    },
    types::{Contract, PublicKeyHash, SmartRollupAddress},
    utils::inbox::file::{InboxFile, Message},
};
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

// tag + 20 byte address
const EXTERNAL_FRAME_SIZE: usize = 21;
const DEFAULT_GAS_LIMIT: u32 = 100_000;
const MNEMONIC: &str =
    "donate kidney style loyal nose core inflict cup symptom speed giant polar";
// FIXME: JSTZ-854
type DepositInboxMsgPayloadType = MichelsonOr<
    MichelsonPair<MichelsonContract, FA2_1Ticket>,
    MichelsonPair<
        MichelsonContract,
        MichelsonPair<MichelsonOption<MichelsonContract>, FA2_1Ticket>,
    >,
>;

pub struct Account {
    nonce: Nonce,
    sk: SecretKey,
    pk: PublicKey,
    pub address: Address,
}

pub struct InboxBuilder {
    messages: Vec<Message>,
    rollup_address: SmartRollupAddress,
    next_account_id: usize,
    ticketer_address: Option<ContractKt1Hash>,
}

impl InboxBuilder {
    pub fn new(
        rollup_address: SmartRollupAddress,
        ticketer_address: Option<ContractKt1Hash>,
    ) -> Self {
        Self {
            rollup_address,
            messages: Vec::new(),
            next_account_id: 0,
            ticketer_address,
        }
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

    fn generate_internal_messge<T: Michelson>(
        &self,
        m: InternalInboxMessage<T>,
    ) -> Result<Message> {
        let msg = InboxMessage::Internal(m);
        let mut bytes = Vec::new();
        msg.serialize(&mut bytes)?;
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
        body: Option<Vec<u8>>,
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

    fn deposit_payload(
        ticketer: &ContractKt1Hash,
        account: &Account,
        amount_mutez: u64,
    ) -> DepositInboxMsgPayloadType {
        MichelsonOr::Left(MichelsonPair(
            MichelsonContract(Contract::Implicit(
                PublicKeyHash::from_b58check(&account.address.to_string())
                    .expect("serialised address should be parsable"),
            )),
            Ticket::new(
                Contract::Originated(ticketer.clone()),
                MichelsonPair(MichelsonNat::from(0), MichelsonOption(None)),
                amount_mutez,
            )
            .expect("ticket creation from ticketer should work"),
        ))
    }

    pub fn deposit_from_l1(
        &mut self,
        account: &Account,
        amount_mutez: u64,
    ) -> Result<()> {
        match &self.ticketer_address {
            Some(ticketer) => {
                let message = self.generate_internal_messge(
                    InternalInboxMessage::Transfer(Transfer {
                        sender: ticketer.clone(),
                        // any user address is okay here since L1 is not really involved
                        source: PublicKeyHash::from_b58check(
                            "tz1W8rEphWEjMcD1HsxEhsBFocfMeGsW7Qxg",
                        )
                        .expect("the constant source address should be parsable"),
                        destination: self.rollup_address.clone(),
                        payload: Self::deposit_payload(ticketer, account, amount_mutez),
                    }),
                )?;
                self.messages.push(message);
                Ok(())
            }
            None => Err("ticketer address is not provided".into()),
        }
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
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup::{
        inbox::{ExternalMessageFrame, InboxMessage, InternalInboxMessage},
        michelson::{MichelsonOr, MichelsonUnit},
        types::SmartRollupAddress,
        utils::inbox::file::Message,
    };

    use super::{DepositInboxMsgPayloadType, InboxBuilder};

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
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let mut builder = InboxBuilder::new(rollup_address, None);
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
        let mut builder = InboxBuilder::new(rollup_address.clone(), None);
        builder
            .run_function(
                &mut default_account(),
                Uri::try_from("jstz://foobar/transfer".to_string()).unwrap(),
                Method::GET,
                HeaderMap::new(),
                None,
            )
            .unwrap();
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.first().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(raw).unwrap();
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
        let mut builder = InboxBuilder::new(rollup_address.clone(), None);
        builder
            .deploy_function(&mut default_account(), ParsedCode("code".to_string()), 123)
            .unwrap();
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.first().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(raw).unwrap();
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
        let builder = InboxBuilder::new(rollup_address.clone(), None);
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
    fn generate_internal_messge() {
        let rollup_address =
            SmartRollupAddress::from_b58check("sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao")
                .unwrap();
        let builder = InboxBuilder::new(rollup_address.clone(), None);
        let message = builder
            .generate_internal_messge(InternalInboxMessage::<MichelsonUnit>::StartOfLevel)
            .unwrap();
        match message {
            Message::Raw(raw) => {
                let (_, inbox_msg) = InboxMessage::<MichelsonUnit>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::Internal(m) => {
                        matches!(m, InternalInboxMessage::StartOfLevel);
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
        let mut builder = InboxBuilder::new(rollup_address.clone(), None);
        let account = builder.create_accounts(1).unwrap().pop().unwrap();
        assert_eq!(
            builder
                .deposit_from_l1(&account, 1)
                .unwrap_err()
                .to_string(),
            "ticketer address is not provided"
        );

        let mut builder = InboxBuilder::new(
            rollup_address.clone(),
            Some(
                ContractKt1Hash::from_base58_check(
                    "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5",
                )
                .unwrap(),
            ),
        );
        builder.deposit_from_l1(&account, 1).unwrap();
        assert_eq!(builder.messages.len(), 1);
        match builder.messages.pop().unwrap() {
            Message::Raw(raw) => {
                let (_, inbox_msg) =
                    InboxMessage::<DepositInboxMsgPayloadType>::parse(&raw).unwrap();
                match inbox_msg {
                    InboxMessage::Internal(InternalInboxMessage::Transfer(transfer)) => {
                        assert_eq!(transfer.destination, builder.rollup_address);
                        assert_eq!(transfer.sender, builder.ticketer_address.unwrap());
                        assert!(matches!(transfer.payload, MichelsonOr::Left(_)));
                    }
                    _ => panic!("should be internal message"),
                }
            }
            _ => panic!("should be raw message"),
        }
    }
}
