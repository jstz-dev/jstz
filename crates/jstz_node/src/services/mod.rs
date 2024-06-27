mod accounts;
pub mod logs;
mod operations;

pub use accounts::AccountsService;
use actix_web::web::ServiceConfig;
pub use logs::LogsService;
pub use operations::OperationsService;

pub trait Service {
    fn configure(cfg: &mut ServiceConfig);
}
