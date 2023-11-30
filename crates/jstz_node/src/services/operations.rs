use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_proto::receipt::Receipt;
use octez::OctezRollupClient;

use crate::Result;

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

impl OperationsService {
    pub fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/operations").service(receipt);

        cfg.service(scope);
    }
}
