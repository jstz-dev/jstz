use actix_web::{
    get, post,
    web::{self, Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_proto::{operation::SignedOperation, receipt::Receipt};
use octez::OctezRollupClient;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;

use crate::Result;

use super::Service;

#[post("")]
async fn inject(
    rollup_client: Data<OctezRollupClient>,
    operation: web::Json<SignedOperation>,
) -> Result<impl Responder> {
    let encoded_operation = bincode::serialize(&operation)
        .map_err(|_| anyhow!("Failed to serialize operation"))?;

    let address = rollup_client.get_rollup_address().await?;

    let message_frame = ExternalMessageFrame::Targetted {
        address,
        contents: encoded_operation,
    };

    let mut binary_contents = Vec::new();
    message_frame
        .bin_write(&mut binary_contents)
        .map_err(|_| anyhow!("Failed to write binary frame"))?;

    rollup_client.batcher_injection([binary_contents]).await?;

    Ok(HttpResponse::Ok())
}

#[get("/{hash}/receipt")]
async fn receipt(
    rollup_client: Data<OctezRollupClient>,
    path: Path<String>,
) -> Result<impl Responder> {
    let key = format!("/jstz_receipt/{}", path.into_inner());

    let value = rollup_client.get_value(&key).await?;

    let receipt = match value {
        Some(value) => bincode::deserialize::<Receipt>(&value)
            .map_err(|_| anyhow!("Failed to deserialize receipt"))?,
        None => return Ok(HttpResponse::NotFound().finish()),
    };

    Ok(HttpResponse::Ok().json(receipt))
}

pub struct OperationsService;

impl Service for OperationsService {
    fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/operations").service(inject).service(receipt);

        cfg.service(scope);
    }
}
