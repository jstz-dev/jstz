use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};

use crate::{rollup::RollupClient, Result};

#[get("/{hash}/receipt")]
async fn receipt(
    rollup_client: Data<RollupClient>,
    path: Path<String>,
) -> Result<impl Responder> {
    let key = format!("/jstz_receipt/{}", path.into_inner());

    let value = rollup_client.get_value(&key).await?;

    Ok(HttpResponse::Ok().json(value))
}

pub struct OperationsSerivce;

impl OperationsSerivce {
    pub fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/operations").service(receipt);

        cfg.service(scope);
    }
}
