use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    HttpResponse, Responder, Scope,
};
use anyhow::anyhow;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Account;

use crate::{rollup::RollupClient, Result};

#[get("/{address}/nonce")]
async fn nonce(
    rollup_client: Data<RollupClient>,
    path: Path<String>,
) -> Result<impl Responder> {
    let address = path.into_inner();

    let address_path = Account::path(
        &PublicKeyHash::from_base58(&address).expect("Failed to create address"),
    )
    .expect("Failed to get account path");

    let value = rollup_client.get_value(&address_path.to_string()).await?;

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
