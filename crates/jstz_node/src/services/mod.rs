use actix_web::web::ServiceConfig;

pub mod accounts;
pub mod logs;
pub mod operations;

pub use accounts::AccountsService;
pub use logs::LogsService;
pub use operations::OperationsService;

pub trait Service {
    fn configure(cfg: &mut ServiceConfig);
}
