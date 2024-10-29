use crate::AppState;

use axum::Router;

pub mod accounts;
pub mod error;
pub mod logs;
pub mod operations;

pub trait Service {
    fn router() -> Router<AppState>;
}
