use crate::services::AppState;
use axum::{extract::State, response::IntoResponse};

pub async fn get_mode(
    State(AppState { mode, .. }): State<AppState>,
) -> impl IntoResponse {
    serde_json::to_string(&mode).unwrap().into_response()
}
