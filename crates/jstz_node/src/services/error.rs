use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use derive_more::From;
use serde_json::json;

#[derive(From)]
pub enum ServiceError {
    FromAnyhow(anyhow::Error),
    NotFound,
    BadRequest(String),
}

pub type ServiceResult<T> = anyhow::Result<T, ServiceError>;

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        match self {
            ServiceError::FromAnyhow(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error_body(error)).into_response()
            }
            ServiceError::NotFound => StatusCode::NOT_FOUND.into_response(),
            ServiceError::BadRequest(error) => {
                (StatusCode::BAD_REQUEST, error_body(error)).into_response()
            }
        }
    }
}

fn error_body(e: impl ToString) -> Body {
    let json = json!({ "error": e.to_string() });
    Body::from(serde_json::to_vec(&json).unwrap())
}
