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
    print!("nonce ");
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

#[get("/{address}/kv/{key:.*}")]
async fn kv(
    rollup_client: Data<RollupClient>,
    path: Path<(String, String)>,
) -> Result<impl Responder> {
    let (address, key) = path.into_inner();

    let storage_key = if key.is_empty() {
        format!("/jstz_kv/{}", address)
    } else {
        format!("/jstz_kv/{}/{}", address, key)
    };

    let value = rollup_client.get_value(&storage_key).await?;

    println!("value: {:?}", value);

    let value = match value {
        Some(value) => bincode::deserialize::<KvValue>(&value)
            .map_err(|_| anyhow!("Failed to deserialize account"))?,
        None => return Ok(HttpResponse::NotFound().finish()),
    };

    Ok(HttpResponse::Ok().json(value))
}

#[get("/{address}/kv_subkeys/{key:.*}")]
async fn kv_subkeys(
    rollup_client: Data<RollupClient>,
    path: Path<(String, String)>,
) -> Result<impl Responder> {
    let (address, key) = path.into_inner();

    let storage_key = if key.is_empty() {
        format!("/jstz_kv/{}", address)
    } else {
        format!("/jstz_kv/{}/{}", address, key)
    };

    let value = rollup_client.get_subkeys(&storage_key).await?;

    let value = match value {
        Some(value) => value,
        None => return Ok(HttpResponse::NotFound().finish()),
    };

    Ok(HttpResponse::Ok().json(value))
}

pub struct AccountsService;

impl AccountsService {
    pub fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/accounts")
            .service(nonce)
            .service(kv)
            .service(kv_subkeys);

        cfg.service(scope);
    }
}
