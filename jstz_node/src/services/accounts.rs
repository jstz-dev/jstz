use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_proto::context::account::Account;

use crate::{rollup::RollupClient, Result};

#[get("/{address}/nonce")]
async fn nonce(
    rollup_client: Data<RollupClient>,
    path: Path<String>,
) -> Result<impl Responder> {
    let key = format!("/jstz_account/{}", path.into_inner());

    let value = rollup_client.get_value(&key).await?;

    let nonce = match value {
        Some(value) => {
            bincode::deserialize::<Account>(&value)
                .map_err(|_| anyhow!("Failed to deserialize account"))?
                .nonce
        }
        None => return Ok(HttpResponse::NotFound().finish()),
    };

    Ok(HttpResponse::Ok().json(nonce))
}

pub struct AccountsService;

impl AccountsService {
    pub fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/accounts").service(nonce);

        cfg.service(scope);
    }
}
