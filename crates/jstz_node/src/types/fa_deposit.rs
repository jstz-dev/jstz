use crate::types::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Amount;
use jstz_proto::executor::fa_deposit::FaDepositReceipt as FaDepositReceiptInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(FaDepositReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "_type")]
pub struct FaDepositReceipt {
    pub receiver: PublicKeyHash,
    pub ticket_balance: Amount,
    pub run_function: Option<crate::types::receipt::RunFunctionReceipt>,
}
