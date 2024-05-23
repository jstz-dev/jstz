use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use jstz_crypto::Error as CryptoError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("An unspecified internal error occurred: {0}")]
    InternalError(#[from] anyhow::Error),
    #[error("Invalid address: {0}")]
    InvalidInput(#[from] CryptoError),
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match &self {
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
