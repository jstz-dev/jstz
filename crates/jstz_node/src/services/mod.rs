use crate::AppState;

use utoipa_axum::router::OpenApiRouter;

pub mod accounts;
pub mod error;
pub mod logs;
pub mod operations;

pub trait Service {
    fn router_with_openapi() -> OpenApiRouter<AppState>;
}
