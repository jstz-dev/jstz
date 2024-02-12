use actix_web::web::ServiceConfig;

mod accounts;
pub mod logs;
mod operations;

pub use accounts::AccountsService;
pub use logs::LogsService;
pub use operations::OperationsService;

pub trait Service {
    fn configure(cfg: &mut ServiceConfig);
}
