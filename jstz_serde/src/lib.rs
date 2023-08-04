use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};
/*
Notes:

Here when we create a contract we just pass in the creating address, but this is insecure
We will need some sort of signing mechanism to ensure that contracts

* Externally created contracts are uploaded through the reveal channel, The address is then a signed



*/

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum InboxMessage {
    #[cfg(feature = "create_contract")]
    CreateContract {
        from_address: String,
        code: String,
        amount: u64,
    },
    #[cfg(feature = "call_contract")]
    CallContract {
        from_address: String,
        parameter: String,
        amount: u64,
    },
    #[cfg(feature = "run_js")]
    RunJs { code: String },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct OutboxMessage {
    pub destination: String,
    pub parameters: String,
    pub entrypoint: Option<String>,
    pub amount: u64,
}

#[derive(Serialize)]
struct InboxRepresentation {
    external: String,
}

impl InboxMessage {
    pub fn to_bytestr(&self) -> String {
        let mut result = "".to_string();
        for byte in to_stdvec(self).unwrap().into_iter() {
            result.push_str(&format!("{:02x}", byte))
        }

        result
    }
}
impl TryFrom<&[u8]> for InboxMessage {
    type Error = postcard::Error;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        from_bytes(bytes)
    }
}
impl OutboxMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        panic!()
    }
}

pub fn into_inbox_array<I: IntoIterator<Item = InboxMessage>>(iter: I) -> String {
    let v: Vec<_> = iter
        .into_iter()
        .map(|msg| InboxRepresentation {
            external: msg.to_bytestr(),
        })
        .collect();
    serde_json::to_string(&vec![v]).unwrap()
}
