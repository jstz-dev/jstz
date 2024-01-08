use actix_web::{
    get, post,
    web::{self, Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_proto::receipt::Receipt;
use octez::OctezRollupClient;

use crate::Result;

use super::Service;

#[post("")]
async fn inject(
    rollup_client: Data<OctezRollupClient>,
    operation: web::Bytes,
) -> Result<impl Responder> {
    // FIXME: @johnyob
    // The operation should be deserialized from JSON here and serialized to an internal format
    // But it seems that there is a serde issue with JSON serialization + deserialization of BLS signatures
    // So for now we just pass the raw bytes to the rollup node.

    rollup_client.batcher_injection([operation]).await?;

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
