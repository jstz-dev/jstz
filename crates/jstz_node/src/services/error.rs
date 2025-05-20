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
    PersistentLogsDisabled,
    ServiceUnavailable(Option<anyhow::Error>),
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
            ServiceError::PersistentLogsDisabled => {
                ServiceError::BadRequest("Persistent logs disabled".to_string())
                    .into_response()
            }
            ServiceError::ServiceUnavailable(error) => match error {
                Some(e) => {
                    (StatusCode::SERVICE_UNAVAILABLE, error_body(e)).into_response()
                }
                None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            },
        }
    }
}

fn error_body(e: impl ToString) -> Body {
    let json = json!({ "error": e.to_string() });
    Body::from(serde_json::to_vec(&json).unwrap())
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::ServiceError;

    #[tokio::test]
    async fn service_unavailable() {
        async fn check(inner: Option<anyhow::Error>, expected: &str) {
            let res = ServiceError::ServiceUnavailable(inner).into_response();
            assert_eq!(res.status(), 503);
            let body = String::from_utf8(
                to_bytes(res.into_body(), 1000).await.unwrap().to_vec(),
            )
            .unwrap();
            assert_eq!(body, expected);
        }

        check(None, "").await;
        check(Some(anyhow::anyhow!("foobar")), "{\"error\":\"foobar\"}").await;
    }
}
