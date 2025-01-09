use crate::types::public_key_hash::PublicKeyHash;
use jstz_proto::executor::fa_withdraw::FaWithdrawReceipt as FaWithdrawReceiptInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

type OutboxMessageId = String;

#[api_map_to(FaWithdrawReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(tag = "_type")]
pub struct FaWithdrawReceipt {
    pub source: PublicKeyHash,
    pub outbox_message_id: OutboxMessageId,
}
