use super::ByteRep;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum InboxMessage {
    #[cfg(feature = "create_contract")]
    CreateContract {
        from_address: Address,
        #[serde(flatten)]
        contract: Contract,
    },
    #[cfg(feature = "call_contract")]
    CallContract {
        from_address: Address,
        contract_address: Address,
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

pub fn into_inbox_array<I: IntoIterator<Item = InboxMessage>>(iter: I) -> Option<String> {
    let v: Vec<_> = iter
        .into_iter()
        .map(|msg| InboxRepresentation {
            external: ByteRep::from_t(&msg).to_string(),
        })
        .collect();
    serde_json::to_string(&vec![v]).ok()
}
