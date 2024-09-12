pub mod command;
pub mod joinhandle;

use async_trait::async_trait;
use joinhandle::JoinHandle;
use tokio::signal::unix::Signal;
use tokio::sync::mpsc;

#[async_trait]
pub trait Task {
    /// A set of tasks which are all executed on the same 'execution backend'
    type TaskSet;

    /// Performs the unit of work for the task
    async fn run_task(
        self,
        signals: mpsc::Receiver<Signal>,
        task_group: &Self::TaskSet,
    ) -> anyhow::Result<()>;

    /// Spawns the task
    async fn spawn_task(self) -> anyhow::Result<JoinHandle>;
}
