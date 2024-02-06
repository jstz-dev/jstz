use actix_web::{
    get, post,
    web::{self, Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_proto::{operation::SignedOperation, receipt::Receipt};
use octez::OctezRollupClient;

use crate::Result;

use super::Service;

#[post("")]
async fn inject(
    rollup_client: Data<OctezRollupClient>,
    operation: web::Json<SignedOperation>,
) -> Result<impl Responder> {
    let encoded_operation = bincode::serialize(&operation)
        .map_err(|_| anyhow!("Failed to serialize operation"))?;

    rollup_client.batcher_injection([encoded_operation]).await?;

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
