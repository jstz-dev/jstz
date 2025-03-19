use std::fs;
use std::path::PathBuf;

use super::error::{ServiceError, ServiceResult};
use super::{AppState, Service};
use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    Json,
};

use jstz_core::reveal_data::{PreimageHash, RevealData, MAX_REVEAL_SIZE};
use jstz_core::BinEncodable;
use jstz_crypto::hash::{Blake2b, Hash};
use jstz_crypto::public_key::PublicKey;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::secret_key::SecretKey;
use jstz_crypto::signature::Signature;
use jstz_proto::context::account::Nonce;
use jstz_proto::operation::{
    Content, Operation, RevealLargePayloadOperation, RevealType, SignedOperation,
};
use jstz_proto::receipt::Receipt;
use octez::OctezRollupClient;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub struct OperationsService;

const OPERATIONS_TAG: &str = "Operations";

type HexEncodedOperationHash = String;

// fn deserialize_account(data: &[u8]) -> ServiceResult<Account> {
//     Ok(Account::decode(data).map_err(|_| anyhow!("Failed to deserialize account"))?)
// }

// async fn get_nonce(rollup_client: &OctezRollupClient) -> Nonce {
//     let key = format!("/jstz_account/{}", "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx");
//     let value = rollup_client.get_value(&key).await.unwrap();
//     let nonce = match value {
//         Some(value) => {
//             let account = deserialize_account(value.as_slice());
//             match account.ok().unwrap() {
//                 Account::User(user) => user.nonce,
//                 Account::SmartFunction(smart_function) => smart_function.nonce,
//             }
//         }
//         None => Nonce::default(),
//     };
//     nonce
// }

fn sign_with_bootstrap1(hash: Blake2b) -> (PublicKey, Signature) {
    // public_key_hash = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
    // public_key = "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav";
    // secret_key = "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
    let sk =
        SecretKey::from_base58("edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh")
            .unwrap();
    let pk =
        PublicKey::from_base58("edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav")
            .unwrap();
    let signature = sk.sign(hash).unwrap();
    (pk, signature)
}

fn handle_large_operation(
    _rollup_client: &OctezRollupClient,
    signed_operation: &SignedOperation,
    preimages_dir: &PathBuf,
) -> Option<SignedOperation> {
    // let op2: SignedOperation = signed_operation.clone();
    let operation: Result<_, _> = signed_operation.clone().verify();
    match operation {
        Ok(operation) => match operation.content {
            Content::DeployFunction(_) => {
                //1. check the size
                let size = signed_operation.encode().unwrap().len();
                if size < 4000 {
                    return None;
                }
                if size > MAX_REVEAL_SIZE {
                    panic!("Operation is too large to be revealed");
                }
                //2. if it's too large, sign into a large payload operation
                let save_preimages = |hash: PreimageHash, preimage: Vec<u8>| {
                    let path = preimages_dir.join(hash.to_string());
                    if let Err(e) = fs::write(&path, preimage) {
                        println!("Failed to write preimage to {:?} due to {}.", path, e);
                    }
                };

                let root_hash: PreimageHash = RevealData::encode_and_prepare_preimages(
                    signed_operation,
                    save_preimages,
                )
                .expect("saving to file should work");
                RevealData::encode_and_prepare_preimages(&operation, save_preimages)
                    .expect("should work");

                println!(
                    "data is avaiable at path: {:?}",
                    preimages_dir.join(hex::encode(root_hash.as_ref()))
                );

                let rdc_op = RevealLargePayloadOperation {
                    root_hash: PreimageHash::from(*root_hash.as_ref()),
                    reveal_type: RevealType::DeployFunction,
                };
                let rdc_op_content = rdc_op;
                // let nonce = get_nonce(_rollup_client).await;
                let rdc_op: Operation = Operation {
                    source: PublicKeyHash::from_base58(
                        "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
                    )
                    .unwrap(),
                    nonce: Nonce(0),
                    content: Content::RevealLargePayloadOperation(rdc_op_content),
                };
                let (pk, sig) = sign_with_bootstrap1(rdc_op.hash());
                let rdc_op_signed = SignedOperation::new(pk, sig, rdc_op);
                return Some(rdc_op_signed);
            }
            _ => None,
        },
        Err(_) => None,
    }
}

/// Inject an operation into Jstz
#[utoipa::path(
        post,
        path = "",
        tag = OPERATIONS_TAG,
        responses(
            (status = 200, description = "Operation successfully injectedd"),
            (status = 400),
            (status = 500)
        )
    )]
async fn inject(
    State(AppState {
        rollup_client,
        rollup_preimages_dir,
        ..
    }): State<AppState>,
    Json(operation): Json<SignedOperation>,
) -> ServiceResult<()> {
    let maybe_large_operation =
        handle_large_operation(&rollup_client, &operation, &rollup_preimages_dir);
    println!("large operation: {:?}", maybe_large_operation);
    let encoded_operation = maybe_large_operation
        .clone()
        .unwrap_or(operation)
        .encode()
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
    if let Some(_) = maybe_large_operation {
        println!("large operation injected");
    }
    Ok(())
}

/// Get the receipt of an operation
#[utoipa::path(
        get,
        path = "/{operation_hash}/receipt",
        tag = OPERATIONS_TAG,
        params(
            ("operation_hash" = String, description = "Operation hash")
        ),
        responses(
            (status = 200, body = Receipt),
            (status = 400),
            (status = 500)
        )
    )]
async fn receipt(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(hash): Path<String>,
) -> ServiceResult<Json<Receipt>> {
    let key = format!("/jstz_receipt/{}", hash);

    let value = rollup_client.get_value(&key).await?;

    let receipt = match value {
        Some(value) => Receipt::decode(value.as_slice())
            .map_err(|_| anyhow!("Failed to deserialize receipt"))?,
        None => Err(ServiceError::NotFound)?,
    };

    Ok(Json(receipt))
}

/// Returns the hex encoded hash of an Operation
#[utoipa::path(
        post,
        path = "/hash",
        tag = OPERATIONS_TAG,
        responses(
            (status = 200, body = HexEncodedOperationHash),
            (status = 400),
            (status = 500)
        )
    )]
async fn hash_operation(
    Json(operation): Json<Operation>,
) -> ServiceResult<Json<HexEncodedOperationHash>> {
    Ok(Json(format!("{}", operation.hash())))
}

impl Service for OperationsService {
    fn router_with_openapi() -> OpenApiRouter<AppState> {
        let routes = OpenApiRouter::new()
            .routes(routes!(inject))
            .routes(routes!(receipt))
            .routes(routes!(hash_operation));

        OpenApiRouter::new().nest("/operations", routes)
    }
}
