use serde::{Deserialize, Serialize};

pub mod address;
pub use address::Address;

pub mod contract;
pub use contract::Contract;

pub mod byte_rep;
pub use byte_rep::ByteRep;

pub mod logging;
pub use logging::{create_log_message, ConsoleMessage, ConsolePrefix};

pub mod messages;
pub use messages::{into_inbox_array, InboxMessage, OutboxMessage};

pub trait Byteable: Serialize + for<'a> Deserialize<'a> {}
impl<T: Serialize + for<'a> Deserialize<'a>> Byteable for T {}
