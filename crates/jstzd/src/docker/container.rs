use anyhow::Result;
use bollard::{
    container::{AttachContainerOptions, LogOutput},
    exec::{CreateExecOptions, StartExecOptions, StartExecResults},
    Docker,
};
use bytes::Bytes;
use futures_util::{future, stream::BoxStream, AsyncBufRead, StreamExt, TryStreamExt};
use log::error;
use std::{io, process::Command, sync::Arc};

enum ExecOutput {
    StdOut(Bytes),
    StdErr(Bytes),
}

impl AsRef<[u8]> for ExecOutput {
    fn as_ref(&self) -> &[u8] {
        match self {
            ExecOutput::StdOut(b) => b.as_ref(),
            ExecOutput::StdErr(b) => b.as_ref(),
        }
    }
}

type ExecOutputStream = BoxStream<'static, Result<ExecOutput>>;

type ExecutionId = String;
pub struct ExecResult {
    id: ExecutionId,
    output: ExecOutputStream,
    client: Arc<Docker>,
}

impl ExecResult {
    pub async fn exit_code(&self) -> Result<Option<i64>> {
        self.client
            .inspect_exec(&self.id)
            .await
            .map(|exec_inspect| exec_inspect.exit_code)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn stdout(&mut self) -> anyhow::Result<impl AsyncBufRead + '_> {
        self.filter_output(|o| matches!(o, ExecOutput::StdOut(_)))
    }

    pub fn stderr(&mut self) -> Result<impl AsyncBufRead + '_> {
        self.filter_output(|o| matches!(o, ExecOutput::StdErr(_)))
    }

    fn filter_output<'a, F>(&'a mut self, f: F) -> Result<impl AsyncBufRead + '_>
    where
        F: Fn(&ExecOutput) -> bool + 'a,
    {
        let filtered = self
            .output
            .as_mut()
            .filter(move |o: &std::result::Result<ExecOutput, anyhow::Error>| {
                #[allow(clippy::redundant_closure)]
                future::ready(o.as_ref().is_ok_and(|o| f(o)))
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));
        Ok(filtered.into_async_read())
    }
}

pub struct Container {
    pub id: String,
    client: Option<Arc<Docker>>,
    _private: (),
}

impl Container {
    /// Creates a new container with running `id`
    pub fn new(client: Arc<Docker>, id: String) -> Self {
        Self {
            id,
            client: Some(client),
            _private: (),
        }
    }

    // Starts the container's entrypoint
    pub async fn start(&self) -> Result<()> {
        self.client()?
            .start_container::<String>(&self.id, None)
            .await?;
        Ok(())
    }

    // Stop the container
    pub async fn stop(&self) -> Result<()> {
        self.client()?.stop_container(&self.id, None).await?;
        Ok(())
    }

    // Execute a command in the container
    pub async fn exec(&self, command: Command) -> Result<ExecResult> {
        let (exec_id, start_exec_results) = self.create_and_start_exec(command).await?;
        if let StartExecResults::Attached { output, .. } = start_exec_results {
            let output: ExecOutputStream = output
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
                .filter_map(|o| async { Self::exec_filter_map(o) })
                .boxed();
            return Ok(ExecResult {
                id: exec_id,
                output,
                client: self.client()?.clone(),
            });
        }
        Err(anyhow::anyhow!("Error starting exec"))
    }

    // Creates buffered reader for stdout IO stream
    pub async fn stdout(&self) -> anyhow::Result<impl AsyncBufRead> {
        let options = Some(AttachContainerOptions::<String> {
            stdout: Some(true),
            logs: Some(true),
            stream: Some(true),
            ..AttachContainerOptions::default()
        });
        self.attach_container(options).await
    }

    // Creates buffered reader for stderr IO stream
    pub async fn stderr(&self) -> anyhow::Result<impl AsyncBufRead> {
        let options: Option<AttachContainerOptions<String>> =
            Some(AttachContainerOptions::<String> {
                stderr: Some(true),
                logs: Some(true),
                stream: Some(true),
                ..AttachContainerOptions::default()
            });
        self.attach_container(options).await
    }

    async fn create_and_start_exec(
        &self,
        command: Command,
    ) -> Result<(ExecutionId, StartExecResults)> {
        let cmd_vec: Vec<String> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let config = CreateExecOptions::<String> {
            cmd: Some(cmd_vec),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..CreateExecOptions::default()
        };
        let create_exec_results = self.client()?.create_exec(&self.id, config).await?;
        let exec_id = create_exec_results.id;
        let start_exec_results = self
            .client()?
            .start_exec(
                &exec_id,
                Some(StartExecOptions {
                    detach: false,
                    tty: false,
                    ..Default::default()
                }),
            )
            .await?;
        Ok((exec_id, start_exec_results))
    }

    fn exec_filter_map(
        output: Result<LogOutput, io::Error>,
    ) -> Option<Result<ExecOutput>> {
        match output {
            Ok(LogOutput::StdOut { message }) => Some(Ok(ExecOutput::StdOut(message))),
            Ok(LogOutput::StdErr { message }) => Some(Ok(ExecOutput::StdErr(message))),
            Ok(_) => None,
            Err(e) => Some(Err(anyhow::anyhow!(e))),
        }
    }
    async fn attach_container(
        &self,
        options: Option<AttachContainerOptions<String>>,
    ) -> Result<impl AsyncBufRead> {
        let stream = self
            .client()?
            .attach_container(&self.id, options)
            .await?
            .output
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));
        Ok(stream.into_async_read())
    }

    pub async fn cleanup(&self) -> Result<()> {
        self.stop()
            .await
            .unwrap_or_else(|e| error!("Error stopping container: {}", e));
        match self.client()?.remove_container(&self.id, None).await {
            Ok(_) => {}
            Err(e) => error!("Error removing container {}: {}", self.id, e),
        }
        Ok(())
    }

    fn client(&self) -> Result<&Arc<Docker>> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Client does not exist"))
    }
}
